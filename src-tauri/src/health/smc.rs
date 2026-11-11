//! Read-only access to the System Management Controller (AppleSMC) for fan speeds
//! and temperatures. Only the read selectors are used; nothing is ever written.

use std::ffi::c_void;

use serde::Serialize;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Vers {
    major: u8,
    minor: u8,
    build: u8,
    reserved: u8,
    release: u16,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct PLimit {
    version: u16,
    length: u16,
    cpu: u32,
    gpu: u32,
    mem: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct KeyInfo {
    data_size: u32,
    data_type: u32,
    data_attributes: u8,
}

/// `SMCKeyData_t` as used by the AppleSMC user client.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct KeyData {
    key: u32,
    vers: Vers,
    p_limit: PLimit,
    key_info: KeyInfo,
    result: u8,
    status: u8,
    data8: u8,
    data32: u32,
    bytes: [u8; 32],
}

const _: () = assert!(std::mem::size_of::<KeyData>() == 80);

const KERNEL_INDEX_SMC: u32 = 2;
const CMD_READ_BYTES: u8 = 5;
const CMD_READ_KEYINFO: u8 = 9;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const std::ffi::c_char) -> *mut c_void;
    fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> u32;
    fn IOServiceOpen(service: u32, owning_task: u32, kind: u32, connect: *mut u32) -> i32;
    fn IOServiceClose(connect: u32) -> i32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IOConnectCallStructMethod(
        connection: u32,
        selector: u32,
        input: *const c_void,
        input_size: usize,
        output: *mut c_void,
        output_size: *mut usize,
    ) -> i32;
}

unsafe extern "C" {
    static mach_task_self_: u32;
}

pub struct Smc {
    conn: u32,
}

// The connection is a plain Mach port number; calls on it are thread-safe.
unsafe impl Send for Smc {}

fn key_code(key: &str) -> u32 {
    key.bytes().take(4).fold(0, |acc, b| (acc << 8) | b as u32)
}

fn type_name(code: u32) -> [u8; 4] {
    code.to_be_bytes()
}

/// Decodes the SMC data types used by fan and temperature keys.
fn decode(kind: [u8; 4], bytes: &[u8]) -> Option<f32> {
    let b = |i: usize| *bytes.get(i).unwrap_or(&0);
    match &kind {
        b"flt " => Some(f32::from_le_bytes([b(0), b(1), b(2), b(3)])),
        b"fpe2" => Some(u16::from_be_bytes([b(0), b(1)]) as f32 / 4.0),
        b"sp78" => Some(i16::from_be_bytes([b(0), b(1)]) as f32 / 256.0),
        b"ui8 " => Some(b(0) as f32),
        b"ui16" => Some(u16::from_be_bytes([b(0), b(1)]) as f32),
        b"ui32" => Some(u32::from_be_bytes([b(0), b(1), b(2), b(3)]) as f32),
        _ => None,
    }
}

impl Smc {
    pub fn open() -> Option<Self> {
        // SAFETY: plain IOKit calls; the matching dictionary is consumed by
        // IOServiceGetMatchingService and the service object is released after opening.
        unsafe {
            let matching = IOServiceMatching(c"AppleSMC".as_ptr());
            if matching.is_null() {
                return None;
            }
            let service = IOServiceGetMatchingService(0, matching);
            if service == 0 {
                return None;
            }
            let mut conn = 0u32;
            let rc = IOServiceOpen(service, mach_task_self_, 0, &mut conn);
            IOObjectRelease(service);
            (rc == 0 && conn != 0).then_some(Smc { conn })
        }
    }

    fn call(&self, input: &KeyData) -> Option<KeyData> {
        let mut output = KeyData::default();
        let mut out_size = std::mem::size_of::<KeyData>();
        // SAFETY: both buffers are valid KeyData structs of the size passed in.
        let rc = unsafe {
            IOConnectCallStructMethod(
                self.conn,
                KERNEL_INDEX_SMC,
                (input as *const KeyData).cast(),
                std::mem::size_of::<KeyData>(),
                (&raw mut output).cast(),
                &mut out_size,
            )
        };
        (rc == 0 && output.result == 0).then_some(output)
    }

    /// Reads a key and decodes it as a number.
    pub fn read(&self, key: &str) -> Option<f32> {
        let info = self.call(&KeyData {
            key: key_code(key),
            data8: CMD_READ_KEYINFO,
            ..Default::default()
        })?;
        let size = info.key_info.data_size;
        if size == 0 || size > 32 {
            return None;
        }
        let out = self.call(&KeyData {
            key: key_code(key),
            key_info: KeyInfo {
                data_size: size,
                ..Default::default()
            },
            data8: CMD_READ_BYTES,
            ..Default::default()
        })?;
        decode(
            type_name(info.key_info.data_type),
            &out.bytes[..size as usize],
        )
    }
}

impl Drop for Smc {
    fn drop(&mut self) {
        // SAFETY: conn came from a successful IOServiceOpen.
        unsafe {
            IOServiceClose(self.conn);
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct Fan {
    pub label: String,
    pub rpm: f32,
    pub min: f32,
    pub max: f32,
}

/// `None` when fans cannot be read; an empty list on a fanless Mac.
pub fn fans(smc: &Smc) -> Option<Vec<Fan>> {
    let count = smc.read("FNum")? as usize;
    let fans = (0..count.min(4))
        .filter_map(|i| {
            let rpm = smc.read(&format!("F{i}Ac"))?;
            Some(Fan {
                label: match (count, i) {
                    (2, 0) => "Left fan".into(),
                    (2, 1) => "Right fan".into(),
                    (1, _) => "Fan".into(),
                    _ => format!("Fan {}", i + 1),
                },
                rpm: rpm.max(0.0),
                min: smc.read(&format!("F{i}Mn")).unwrap_or(0.0),
                max: smc.read(&format!("F{i}Mx")).unwrap_or(0.0),
            })
        })
        .collect();
    Some(fans)
}

/// CPU die temperature keys across Intel and Apple silicon generations.
/// Key names are undocumented, so this is best effort: the hottest plausible reading wins.
const CPU_TEMP_KEYS: &[&str] = &[
    // Intel
    "TC0P", "TC0D", "TC0E", "TC0F", // M1
    "Tp09", "Tp0T", "Tp01", "Tp05", "Tp0D", "Tp0H", "Tp0L", "Tp0P", "Tp0X", "Tp0b",
    // M2
    "Tp1h", "Tp1t", "Tp1p", "Tp1l", "Tp0f", "Tp0j", // M3 / M4
    "Te05", "Te0L", "Te0P", "Te0S", "Tf04", "Tf09", "Tf0A", "Tf0B", "Tf0D", "Tf0E", "Tf44", "Tf49",
    "Tf4A", "Tf4B", "Tf4D", "Tf4E",
];

pub fn cpu_temperature(smc: &Smc) -> Option<f32> {
    CPU_TEMP_KEYS
        .iter()
        .filter_map(|k| smc.read(k))
        .filter(|t| (10.0..=125.0).contains(t))
        .fold(None, |best, t| Some(best.map_or(t, |b: f32| b.max(t))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_codes_are_big_endian_ascii() {
        assert_eq!(key_code("FNum"), 0x464E_756D);
        assert_eq!(&type_name(0x666C_7420), b"flt ");
    }

    #[test]
    fn decodes_common_types() {
        assert_eq!(decode(*b"fpe2", &[0x1F, 0x40]), Some(2000.0));
        assert_eq!(decode(*b"flt ", &2150.0f32.to_le_bytes()), Some(2150.0));
        assert_eq!(decode(*b"sp78", &[0x36, 0x80]), Some(54.5));
        assert_eq!(decode(*b"ui8 ", &[2]), Some(2.0));
        assert_eq!(decode(*b"????", &[0]), None);
    }

    #[test]
    fn opening_the_smc_does_not_panic() {
        // Hardware dependent: just make sure the read path is sound where available.
        if let Some(smc) = Smc::open() {
            let _ = fans(&smc);
            let _ = cpu_temperature(&smc);
        }
    }
}
