use rosc::encoder::encode;
use rosc::{OscMessage, OscPacket, OscType};

use std::{thread, time};

use tokio::net::UdpSocket;
use tokio::prelude::*;

use std::time::Instant;
use euclidian_rythms::*;
use audio_thread_priority::promote_current_thread_to_real_time;

const DESTINATION_PORT:u16 = 8000;

/// Utility function: returns an osc packet from a address and arguments
fn build_osc_message(addr: &str, args: Vec<OscType>) -> OscPacket {
    let message = OscMessage {
        addr: addr.to_owned(),
        args: Some(args),
    };
    OscPacket::Message(message)
}

// Utility function: returns a socket ready to use
fn new_bound_socket() -> UdpSocket {
    let mut port = 8000;
    loop {
        let server_addr = format!("127.0.0.1:{}", port).parse().unwrap();
        let bind_result = UdpSocket::bind(&server_addr);
        match bind_result {
            Ok(socket) => break socket,
            Err(_e) => {
                if port > 65535 {
                    panic!("Could not bind socket: port exhausted");
                }
            }
        }
        port += 1;
    }
}

// From a time in ms, and a tempo, return a floating point number: if its decimal part is zero, it's
// right on the beat, otherwise it's in between beats, etc.
fn time_to_beats(time: u128, tempo: f32) -> f32 {
  (time as f32) / 1000. * tempo / 60.
}

fn main() {
    let start = Instant::now();
    let mut socket = new_bound_socket();

    // An array of 12 steps, each element determine whether or not a note should be played at this
    // time. We loop continuously over this.
    let mut sequence = vec![0; 12];
    let pulses = 7;
    euclidian_rythm(&mut sequence, pulses).unwrap();

    // Print the 7;12 euclidian rhythm
    println!("sequence: {:?}", sequence);

    // Out Tempo
    let tempo = 127.0;

    let mut prev: usize = 0;

    // Create a scheduling thread, that is going to be real time, to send our OSC data
    thread::spawn(move || {
        // We send OSC data to localhost at a specific port
        let addr = format!("127.0.0.1:{}", DESTINATION_PORT).parse().unwrap();
        let mut bytes: Vec<u8>;
        let mut i = 0;

        // Promote the current thread to real-time scheduling to have better timing under load
        let handle = match promote_current_thread_to_real_time(512, 44100) {
            Ok(handle) => {
                println!("promoted the scheduling thread to real-time");
                handle
            }
            Err(_) => {
                panic!("error promoting the scheduling thread to real-time")
            }
        };
        loop {
            // Check the duration since the start of the program. Determine which beat is the
            // current beat.
            let now = Instant::now();
            let elapsed = now.duration_since(start).as_millis();

            let beats = time_to_beats(elapsed, tempo);

            // check if this is the first loop iteration after a new beat
            if (beats as usize) != prev {
                prev = beats as usize;

                // Determine where in the sequence this iss
                let sequence_position = beats as usize % sequence.len();

                // if there is a note at this time in the sequence, send it
                if sequence[sequence_position as usize] == 1 {
                    println!("gate at beat {} (time in ms: {})", beats, elapsed);
                    let packet = build_osc_message( "/prefix/note", vec![OscType::Int(i32::from(i))]);
                    i+=1;
                    bytes = encode(&packet).unwrap();
                    socket = socket
                        .send_dgram(bytes, &addr)
                        .wait()
                        .map(|(s, _)| s)
                        .unwrap();
                } else {
                    println!("no gate at beat {} (time in ms: {})", beats, elapsed);
                }
            }
            // a tick is 10ms, could be lower if need be (if the music to play has lots of short events)
            let refresh = time::Duration::from_millis(10);
            thread::sleep(refresh);
        }
    });
    // The main thread does nothing and sleeps. We could have a GUI here that would send message to
    // the scheduling thread.
    loop {
        let refresh = time::Duration::from_millis(100);
        thread::sleep(refresh);
    }
}
