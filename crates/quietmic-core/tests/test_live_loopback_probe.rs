use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[test]
fn test_live_wasapi_loopback_probe() {
    let host = cpal::default_host();
    let dev = match host.default_output_device() {
        Some(d) => d,
        None => {
            println!("No default output device available!");
            return;
        }
    };
    let name = dev.name().unwrap_or_else(|_| "Unknown".to_string());
    println!("Testing default output device: {}", name);

    let cfg_res = dev.default_output_config();
    println!("default_output_config result: {:?}", cfg_res);
    let cfg = match cfg_res {
        Ok(c) => c,
        Err(e) => {
            println!("Cannot get default_output_config: {}", e);
            return;
        }
    };

    println!(
        "Config details: sample_rate={}, channels={}, sample_format={:?}",
        cfg.sample_rate().0,
        cfg.channels(),
        cfg.sample_format()
    );

    let stream_res = dev.build_input_stream(
        &cfg.clone().into(),
        move |data: &[f32], _| {
            println!("Received {} loopback samples", data.len());
        },
        move |err| {
            println!("Loopback stream error callback: {}", err);
        },
        None,
    );

    match stream_res {
        Ok(stream) => {
            println!("build_input_stream SUCCESS! Calling play()...");
            match stream.play() {
                Ok(_) => {
                    println!("stream.play() SUCCESS! Waiting 300ms...");
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }
                Err(e) => {
                    println!("stream.play() FAILED: {}", e);
                }
            }
        }
        Err(e) => {
            println!("build_input_stream FAILED: {}", e);
        }
    }
}
