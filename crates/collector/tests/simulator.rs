use collector::parse_ec_frame;
use common::{Quality, SourceId};
use simulator_ezo_ec::EzoEcCore;

#[test]
fn parses_simulator_periodic_frame() {
    let mut simulator = EzoEcCore::new();
    let frame = simulator.periodic_frame();

    let sample = parse_ec_frame(&frame).expect("simulator frame should be valid");

    assert_eq!(sample.ec_us_cm, Some(450.0));
    assert_eq!(sample.temp_c, None);
    assert_eq!(sample.source, SourceId::EzoEc);
    assert_eq!(sample.quality, Quality::Ok);
}
