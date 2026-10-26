#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Hello(Vec<u8>),
    Auth(Vec<u8>),
    Config(Vec<u8>),
    Packet(Vec<u8>),
    Ping(u64),
    Close { code: u16, reason: String },
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid TAVRA Link frame")]
pub struct CodecError;
const MAX_PAYLOAD: usize = 65_535;
const MAX_CONTROL: usize = 4_096;
impl Frame {
    pub fn encode(&self) -> Result<Vec<u8>, CodecError> {
        let (kind, payload) = match self {
            Self::Hello(v) => {
                if v.len() != 32 {
                    return Err(CodecError);
                };
                (1, v.clone())
            }
            Self::Auth(v) | Self::Config(v) => {
                if v.len() > MAX_CONTROL {
                    return Err(CodecError);
                };
                (if matches!(self, Self::Auth(_)) { 2 } else { 3 }, v.clone())
            }
            Self::Packet(v) => {
                validate_ip(v)?;
                (4, v.clone())
            }
            Self::Ping(v) => (5, v.to_be_bytes().to_vec()),
            Self::Close { code, reason } => {
                if reason.len() > 256 {
                    return Err(CodecError);
                };
                let mut p = code.to_be_bytes().to_vec();
                p.extend_from_slice(reason.as_bytes());
                (6, p)
            }
        };
        if payload.len() > MAX_PAYLOAD {
            return Err(CodecError);
        }
        let mut result = vec![0x54, 0x56, 0, kind];
        result.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        result.extend_from_slice(&payload);
        Ok(result)
    }
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        if input.len() < 8 || input[..3] != [0x54, 0x56, 0] {
            return Err(CodecError);
        }
        let size = u32::from_be_bytes(input[4..8].try_into().map_err(|_| CodecError)?) as usize;
        if size > MAX_PAYLOAD || input.len() != 8 + size {
            return Err(CodecError);
        }
        let p = &input[8..];
        let frame = match input[3] {
            1 if p.len() == 32 => Self::Hello(p.to_vec()),
            2 if p.len() <= MAX_CONTROL => Self::Auth(p.to_vec()),
            3 if p.len() <= MAX_CONTROL => Self::Config(p.to_vec()),
            4 => {
                validate_ip(p)?;
                Self::Packet(p.to_vec())
            }
            5 if p.len() == 8 => {
                Self::Ping(u64::from_be_bytes(p.try_into().map_err(|_| CodecError)?))
            }
            6 if (2..=258).contains(&p.len()) => Self::Close {
                code: u16::from_be_bytes([p[0], p[1]]),
                reason: std::str::from_utf8(&p[2..])
                    .map_err(|_| CodecError)?
                    .to_owned(),
            },
            _ => return Err(CodecError),
        };
        Ok(frame)
    }
}
pub fn validate_ip(p: &[u8]) -> Result<(), CodecError> {
    match p.first().map(|v| v >> 4) {
        Some(4) if p.len() >= 20 => {
            let h = (p[0] & 15) as usize * 4;
            let n = u16::from_be_bytes([p[2], p[3]]) as usize;
            if h < 20 || h > p.len() || n != p.len() {
                return Err(CodecError);
            }
        }
        Some(6) if p.len() >= 40 => {
            let n = u16::from_be_bytes([p[4], p[5]]) as usize;
            if n + 40 != p.len() {
                return Err(CodecError);
            }
        }
        _ => return Err(CodecError),
    };
    if p.len() > MAX_PAYLOAD {
        return Err(CodecError);
    }
    Ok(())
}
