use crate::{Quality, TelemetrySample};
use std::time::Duration;
use time::OffsetDateTime;

/// Severity assigned by the host to a rule violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Critical,
}

/// Lower and upper limits for one measurement. Equality is within the safe range.
/// `None` disables that limit. An empty rule still detects a missing value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThresholdRule {
    pub minimum: Option<f32>,
    pub maximum: Option<f32>,
    pub severity: Severity,
}

/// Optional absolute and relative change limits for one measurement.
/// A spike is reported when either configured limit is reached.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpikeRule {
    pub absolute: Option<f32>,
    pub relative: Option<f32>,
    pub severity: Severity,
}

/// Explicit host-supplied rules. There are no aquarium-specific defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RulesConfig {
    pub ec: Option<ThresholdRule>,
    pub temperature: Option<ThresholdRule>,
    pub max_age: Option<Duration>,
    pub stale_severity: Severity,
    pub spike: Option<SpikeRule>,
}

/// A pure evaluator for normalized telemetry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RulesEngine {
    config: RulesConfig,
}

impl RulesEngine {
    pub const fn new(config: RulesConfig) -> Self {
        Self { config }
    }

    pub const fn config(&self) -> RulesConfig {
        self.config
    }

    /// Evaluates without I/O or wall-clock access. Invalid-quality samples are
    /// ignored, and a previous sample is used only when it has the same tank.
    pub fn evaluate(
        &self,
        sample: &TelemetrySample,
        previous: Option<&TelemetrySample>,
        now: OffsetDateTime,
    ) -> Vec<AlarmEvent> {
        if sample.quality != Quality::Ok {
            return Vec::new();
        }

        let mut events = Vec::new();
        if let Some(max_age) = self.config.max_age {
            let age = now.unix_timestamp_nanos() - sample.timestamp.unix_timestamp_nanos();
            if age > 0 && age as u128 > max_age.as_nanos() {
                events.push(self.event(
                    sample,
                    "measurement.stale",
                    self.config.stale_severity,
                    None,
                    format!("sample is older than {max_age:?}"),
                ));
            }
        }

        evaluate_threshold(
            &mut events,
            sample,
            "ec",
            sample.ec_us_cm,
            self.config.ec,
            self,
        );
        evaluate_threshold(
            &mut events,
            sample,
            "temperature",
            sample.temp_c,
            self.config.temperature,
            self,
        );

        if let Some(spike) = self.config.spike
            && let Some(previous) = previous.filter(|candidate| {
                candidate.tank_id == sample.tank_id && candidate.quality == Quality::Ok
            })
        {
            evaluate_spike(
                &mut events,
                sample,
                "ec",
                sample.ec_us_cm,
                previous.ec_us_cm,
                spike,
                self,
            );
            evaluate_spike(
                &mut events,
                sample,
                "temperature",
                sample.temp_c,
                previous.temp_c,
                spike,
                self,
            );
        }
        events
    }

    fn event(
        &self,
        sample: &TelemetrySample,
        rule_id: &str,
        severity: Severity,
        observed_value: Option<f32>,
        reason: String,
    ) -> AlarmEvent {
        AlarmEvent {
            tank_id: sample.tank_id.clone(),
            rule_id: rule_id.to_owned(),
            severity,
            reason,
            observed_value,
            timestamp: sample.timestamp,
        }
    }
}

/// A rule violation emitted by [`RulesEngine`].
#[derive(Debug, Clone, PartialEq)]
pub struct AlarmEvent {
    pub tank_id: String,
    pub rule_id: String,
    pub severity: Severity,
    pub reason: String,
    pub observed_value: Option<f32>,
    pub timestamp: OffsetDateTime,
}

fn evaluate_threshold(
    events: &mut Vec<AlarmEvent>,
    sample: &TelemetrySample,
    name: &str,
    value: Option<f32>,
    rule: Option<ThresholdRule>,
    engine: &RulesEngine,
) {
    let Some(rule) = rule else { return };
    let Some(value) = value.filter(|value| value.is_finite()) else {
        events.push(engine.event(
            sample,
            &format!("{name}.missing"),
            rule.severity,
            None,
            format!("{name} measurement is missing"),
        ));
        return;
    };
    if rule.minimum.is_some_and(|minimum| value < minimum) {
        events.push(engine.event(
            sample,
            &format!("{name}.minimum"),
            rule.severity,
            Some(value),
            format!("{name} value {value} is below minimum"),
        ));
    }
    if rule.maximum.is_some_and(|maximum| value > maximum) {
        events.push(engine.event(
            sample,
            &format!("{name}.maximum"),
            rule.severity,
            Some(value),
            format!("{name} value {value} is above maximum"),
        ));
    }
}

fn evaluate_spike(
    events: &mut Vec<AlarmEvent>,
    sample: &TelemetrySample,
    name: &str,
    current: Option<f32>,
    previous: Option<f32>,
    rule: SpikeRule,
    engine: &RulesEngine,
) {
    let (Some(current), Some(previous)) = (current, previous) else {
        return;
    };
    if !current.is_finite() || !previous.is_finite() {
        return;
    }
    let change = (current - previous).abs();
    let relative = if previous == 0.0 {
        None
    } else {
        Some(change / previous.abs())
    };
    if rule.absolute.is_some_and(|limit| change >= limit)
        || rule
            .relative
            .is_some_and(|limit| relative.is_some_and(|value| value >= limit))
    {
        events.push(engine.event(
            sample,
            &format!("{name}.spike"),
            rule.severity,
            Some(current),
            format!("{name} changed by {change} from previous value"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SourceId, TelemetrySample};

    fn sample(timestamp: OffsetDateTime, ec: Option<f32>, temp: Option<f32>) -> TelemetrySample {
        TelemetrySample {
            tank_id: "tank-1".into(),
            timestamp,
            ec_us_cm: ec,
            temp_c: temp,
            source: SourceId::Hardware,
            quality: Quality::Ok,
        }
    }
    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_000).unwrap()
    }
    fn engine(config: RulesConfig) -> RulesEngine {
        RulesEngine::new(config)
    }
    fn threshold(minimum: Option<f32>, maximum: Option<f32>, severity: Severity) -> ThresholdRule {
        ThresholdRule {
            minimum,
            maximum,
            severity,
        }
    }
    fn base() -> RulesConfig {
        RulesConfig {
            ec: None,
            temperature: None,
            max_age: None,
            stale_severity: Severity::Warning,
            spike: None,
        }
    }

    #[test]
    fn thresholds_allow_boundaries_and_report_violations_with_severity() {
        let mut config = base();
        config.ec = Some(threshold(Some(10.0), Some(20.0), Severity::Critical));
        let evaluator = engine(config);
        assert!(
            evaluator
                .evaluate(&sample(now(), Some(10.0), None), None, now())
                .is_empty()
        );
        assert!(
            evaluator
                .evaluate(&sample(now(), Some(20.0), None), None, now())
                .is_empty()
        );
        let events = evaluator.evaluate(&sample(now(), Some(9.0), None), None, now());
        assert_eq!(events[0].rule_id, "ec.minimum");
        assert_eq!(events[0].severity, Severity::Critical);
        assert_eq!(events[0].observed_value, Some(9.0));
        assert_eq!(
            evaluator.evaluate(&sample(now(), Some(21.0), None), None, now())[0].rule_id,
            "ec.maximum"
        );
    }

    #[test]
    fn missing_field_emits_configured_rule_event() {
        let mut config = base();
        config.temperature = Some(threshold(None, Some(30.0), Severity::Warning));
        let events = engine(config).evaluate(&sample(now(), None, None), None, now());
        assert_eq!(events[0].rule_id, "temperature.missing");
        assert_eq!(events[0].observed_value, None);
    }

    #[test]
    fn invalid_quality_produces_no_events() {
        let mut config = base();
        config.ec = Some(threshold(Some(10.0), None, Severity::Critical));
        let mut value = sample(now() - time::Duration::hours(1), Some(1.0), None);
        value.quality = Quality::Invalid;
        assert!(engine(config).evaluate(&value, None, now()).is_empty());
    }

    #[test]
    fn stale_boundary_is_allowed_and_older_sample_is_stale() {
        let mut config = base();
        config.max_age = Some(Duration::from_secs(10));
        let evaluator = engine(config);
        assert!(
            evaluator
                .evaluate(
                    &sample(now() - time::Duration::seconds(10), None, None),
                    None,
                    now()
                )
                .is_empty()
        );
        assert_eq!(
            evaluator.evaluate(
                &sample(now() - time::Duration::seconds(11), None, None),
                None,
                now()
            )[0]
            .rule_id,
            "measurement.stale"
        );
    }

    #[test]
    fn spike_uses_absolute_or_relative_limit() {
        let mut config = base();
        config.spike = Some(SpikeRule {
            absolute: Some(5.0),
            relative: Some(0.5),
            severity: Severity::Warning,
        });
        let previous = sample(now(), Some(10.0), Some(20.0));
        let current = sample(now(), Some(15.0), Some(31.0));
        let events = engine(config).evaluate(&current, Some(&previous), now());
        assert_eq!(
            events
                .iter()
                .map(|event| event.rule_id.as_str())
                .collect::<Vec<_>>(),
            vec!["ec.spike", "temperature.spike"]
        );
    }
}
