use collector::{combine_samples, parse_ds18b20_frame, parse_ec_frame};
use common::{Quality, SourceId};
use simulator_ezo_ec::EzoEcCore;

#[test]
fn combines_simulator_ec_and_ds18b20_into_normalized_sample() {
    let mut simulator = EzoEcCore::new();
    let ec = parse_ec_frame(&simulator.handle_command("R")).expect("valid simulator R response");
    let temperature = parse_ds18b20_frame(
        "9e 01 4b 46 7f ff 02 10 10 : crc=10 YES\n9e 01 4b 46 7f ff 02 10 10 t=26187\n",
    )
    .expect("valid DS18B20 w1 fixture");

    let sample = combine_samples(ec, temperature);

    assert_eq!(sample.ec_us_cm, Some(450.0));
    assert_eq!(sample.tank_id, "test-tank");
    assert_eq!(sample.temp_c, Some(26.187));
    assert_eq!(sample.source, SourceId::Hardware);
    assert_eq!(sample.quality, Quality::Ok);

    let json = serde_json::to_value(&sample).expect("serialize normalized sample");
    assert_eq!(json["ec_us_cm"], 450.0);
    assert_eq!(json["tank_id"], "test-tank");
    let serialized_temperature = json["temp_c"].as_f64().expect("temperature number");
    assert!((serialized_temperature - 26.187).abs() < 0.000001);
    assert_eq!(json["source"], "hardware");
    assert_eq!(json["quality"], "ok");
}
