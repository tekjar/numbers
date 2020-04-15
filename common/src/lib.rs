use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

pub fn generate_string(payload_size: usize) -> String {
    let rand_string: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(payload_size)
        .collect();

    rand_string
}

pub fn generate_payload(payload_size: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let payload: Vec<u8> = (0..payload_size).map(|_| rng.gen_range(0, 255)).collect();
    payload
}
