//! Types shared between lib.rs and m34_tpm2.rs
use std::os::raw::{c_uint, c_uchar};

/// M34: TPM 2.0 Status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tpm2Status {
    Ok,
    Error,
    NotFound,
    Unavailable,
    AuthError,
}

/// M34: TPM 2.0 Error
#[derive(Debug, Clone)]
pub struct Tpm2Error {
    pub code: c_uint,
    pub message: &'static str,
}

/// M34: TPM 2B Name (64 bytes max)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Tpm2BName {
    pub size: c_uint,
    pub buffer: [c_uchar; 64],
}

/// M34: TPM 2B Digest (32 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Tpm2BDigest {
    pub size: c_uint,
    pub buffer: [c_uchar; 32],
}

/// M34: TPM2 Capability enum
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tpm2Cap {
    Handles,
    Commands,
    AuditStates,
    TpmProperties,
    TotalCommands,
    Sessions,
    CommandCode,
    NvIndex,
    EccCurves,
    Algorithms,
    SymmetricEncrypt,
    Public,
    ObjectTable,
    TimeAuthorization,
    EccModes,
    Fips,
    Max,
}

/// M34: TPM 2.0 Permission Types
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tpm2Permissions(pub c_uint);
impl Tpm2Permissions {
    pub const SENSITIVE_DATA: Self = Self(0x00000001);
    pub const EXTEND: Self = Self(0x00000002);
    pub const CLEAR: Self = Self(0x00000004);
    pub const WRITE: Self = Self(0x00000008);
    pub const READ: Self = Self(0x00000010);
    pub const CMK: Self = Self(0x00000020);
    pub const EK: Self = Self(0x00000040);
    pub const TPML: Self = Self(0x00000080);
}

/// M34: TPM2 Object Attributes
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tpm2ObjectAttributes(pub c_uint);
impl Tpm2ObjectAttributes {
    pub const USER_WITH_AUTH: Self = Self(0x0001);
    pub const RESTRICTED: Self = Self(0x0002);
    pub const DECRYPT: Self = Self(0x0004);
    pub const USER: Self = Self(0x0008);
    pub const SIGN_ENCRYPT: Self = Self(0x0010);
    pub const FIXED: Self = Self(0x0020);
    pub const FIXED_POSITION: Self = Self(0x0040);
    pub const SENSITIVE: Self = Self(0x0080);
    pub const SENSITIVE_RESET: Self = Self(0x0100);
    pub const EXTEND: Self = Self(0x0200);
    pub const NODA: Self = Self(0x0400);
    pub const RESTRICT: Self = Self(0x0800);
    pub const WRAPPED: Self = Self(0x1000);
    pub const CHARGED: Self = Self(0x2000);
}

/// M34: TPM2 Handle Types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tpm2HandleType {
    Persistent,
    Transient,
    Permanent,
    CapHandles,
    ObjectSession,
    Session,
}

/// M34: TPM2 Session Types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tpm2SessionType {
    Non,
    Owner,
    Platform,
}

/// M34: TPM 2B Max Buffer (512 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Tpm2BMaxBuffer {
    pub size: c_uint,
    pub data: [c_uchar; 512],
}

/// M34: TPM 2B Attestation
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Tpm2BAttestation {
    pub size: c_uint,
    pub data: [c_uchar; 256],
}

/// M34: TPM 2B Public Key (64 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Tpm2BPublicKey {
    pub size: c_uint,
    pub buffer: [c_uchar; 64],
}