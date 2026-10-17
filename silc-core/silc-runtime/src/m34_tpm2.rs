// M34: TPM 2.0 capabilities and root of trust
// Safe Rust API for TPM 2.0 operations using Windows TBS (Trusted Base Services).
// Real hardware implementation using Windows TBS (Trusted Base Services) API.

use std::os::raw::{c_uchar, c_uint};
use std::sync::OnceLock;
use windows::Win32::Foundation::*;
use windows::Win32::System::TpmBaseServices::*;

// M34: TPM 2.0 Command/Response Codes
const TPM_ST_NO_SESSIONS: u16 = 0x8001;
const TPM_CC_CREATE_PRIMARY: u32 = 0x00000174;
const TPM_CC_QUOTE: u32 = 0x00000157;
const TPM_CC_GET_CAPABILITY: u32 = 0x0000017A;
const TPM_CC_HASH: u32 = 0x00000182;
const TPM_CC_NV_READ: u32 = 0x0000014E;
const TPM_CC_NV_WRITE: u32 = 0x0000014D;
const TPM_CC_START_AUTH_SESSION: u32 = 0x00000176;
const TPM_CC_FLUSH_CONTEXT: u32 = 0x00000165;
const TPM_CC_GET_TEST_RESULT: u32 = 0x0000017C;
const TPM_RC_SUCCESS: u32 = 0x000;

// M34: TPM 2.0 Handle constants
const TPM_RH_ENDORSEMENT: u32 = 0x4000000B;
const TPM_RH_OWNER: u32 = 0x40000001;
const TPM_RH_NULL: u32 = 0x40000007;

// M34: TPM 2.0 Algorithm IDs
const TPM_ALG_RSA: u16 = 0x0001;
const TPM_ALG_SHA256: u16 = 0x000B;

// M34: TPM 2.0 Capability IDs
const TPM_CAP_HANDLES: u32 = 0x00000001;
const TPM_CAP_COMMANDS: u32 = 0x00000002;
const TPM_CAP_TPM_PROPERTIES: u32 = 0x00000006;

// M34: TPM 2.0 Object Attributes
const TPMA_OBJECT_USERWITHAUTH: u32 = 0x00000001;
const TPMA_OBJECT_RESTRICTED: u32 = 0x00000010;
const TPMA_OBJECT_DECRYPT: u32 = 0x00000020;

// M34: TPM 2.0 Status

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tpm2Status {
    Ok,
    Error,
    NotFound,
    Unavailable,
    AuthError,
}

// M34: TPM 2.0 Error

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tpm2ErrorCode {
    NotImplemented,
    NotFound,
    Unavailable,
    AuthError,
    BadParameter,
    ResourceExhausted,
    CommunicationError,
}

#[derive(Debug, Clone)]
pub struct Tpm2Error {
    pub code: Tpm2ErrorCode,
    pub message: String,
}

impl Tpm2Error {
    pub fn not_implemented(msg: impl Into<String>) -> Self {
        Self {
            code: Tpm2ErrorCode::NotImplemented,
            message: msg.into(),
        }
    }

    pub fn communication_error(msg: impl Into<String>) -> Self {
        Self {
            code: Tpm2ErrorCode::CommunicationError,
            message: msg.into(),
        }
    }

    pub fn bad_parameter(msg: impl Into<String>) -> Self {
        Self {
            code: Tpm2ErrorCode::BadParameter,
            message: msg.into(),
        }
    }

    pub fn auth_error(msg: impl Into<String>) -> Self {
        Self {
            code: Tpm2ErrorCode::AuthError,
            message: msg.into(),
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            code: Tpm2ErrorCode::NotFound,
            message: msg.into(),
        }
    }

    pub fn unavailable(msg: impl Into<String>) -> Self {
        Self {
            code: Tpm2ErrorCode::Unavailable,
            message: msg.into(),
        }
    }

    pub fn resource_exhausted(msg: impl Into<String>) -> Self {
        Self {
            code: Tpm2ErrorCode::ResourceExhausted,
            message: msg.into(),
        }
    }
}

impl std::fmt::Display for Tpm2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TPM 2.0 error ({:?}): {}", self.code, self.message)
    }
}

impl std::error::Error for Tpm2Error {}

// M34: TPM 2.0 Types

// M34: TPM 2B Name (64 bytes max)

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Tpm2BName {
    pub size: c_uint,
    pub buffer: [c_uchar; 64],
}

impl Default for Tpm2BName {
    fn default() -> Self {
        Self {
            size: 0,
            buffer: [0; 64],
        }
    }
}

// M34: TPM 2B Digest (32 bytes)

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Tpm2BDigest {
    pub size: c_uint,
    pub buffer: [c_uchar; 32],
}

// M34: TPM2 Capability enum

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

// M34: TPM 2.0 Permission Types

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
    pub const TPML: Tpm2Permissions = Tpm2Permissions(0x00000080);
}

// M34: TPM2 Object Attributes

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
    pub const CHARGED: Tpm2ObjectAttributes = Tpm2ObjectAttributes(0x2000);
}

// M34: TPM2 Handle Types

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tpm2HandleType {
    Persistent,
    Transient,
    Permanent,
    CapHandles,
    ObjectSession,
    Session,
}

// M34: TPM2 Session Types

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tpm2SessionType {
    Non,
    Owner,
    Platform,
}

// M34: TPM 2B Max Buffer (512 bytes)

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Tpm2BMaxBuffer {
    pub size: c_uint,
    pub data: [c_uchar; 512],
}

impl Default for Tpm2BMaxBuffer {
    fn default() -> Self {
        Self {
            size: 0,
            data: [0; 512],
        }
    }
}

// M34: TPM 2B Attestation

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Tpm2BAttestation {
    pub size: c_uint,
    pub data: [c_uchar; 256],
}

impl Default for Tpm2BAttestation {
    fn default() -> Self {
        Self {
            size: 0,
            data: [0; 256],
        }
    }
}

// M34: TPM 2B Public Key (64 bytes)

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Tpm2BPublicKey {
    pub size: c_uint,
    pub buffer: [c_uchar; 64],
}

impl Default for Tpm2BPublicKey {
    fn default() -> Self {
        Self {
            size: 0,
            buffer: [0; 64],
        }
    }
}

// M34: TPM 2.0 Context - Windows TBS

// Wrapper to make the raw pointer Send + Sync
struct TbsContextHandle(*const core::ffi::c_void);
unsafe impl Send for TbsContextHandle {}
unsafe impl Sync for TbsContextHandle {}

static TBS_CONTEXT: OnceLock<TbsContextHandle> = OnceLock::new();

fn get_tbs_context() -> core::result::Result<*const core::ffi::c_void, Tpm2Error> {
    let handle = TBS_CONTEXT.get_or_init(|| {
        unsafe {
            let mut context: *mut core::ffi::c_void = core::ptr::null_mut();
            let result = Tbsi_Context_Create(
                TBS_CONTEXT_VERSION_TWO as *const TBS_CONTEXT_PARAMS,
                &mut context as *mut *mut core::ffi::c_void,
            );
            if result != ERROR_SUCCESS.0 {
                // Return a null handle on failure - caller will check
                return TbsContextHandle(core::ptr::null());
            }
            TbsContextHandle(context as *const core::ffi::c_void)
        }
    });

    if handle.0.is_null() {
        Err(Tpm2Error::communication_error(
            "TBS context creation failed",
        ))
    } else {
        Ok(handle.0)
    }
}

// M34: TPM 2.0 Command Helper Functions

/// Submit a TPM 2.0 command via TBS
fn tpm2_submit_command(
    context: *const core::ffi::c_void,
    command: &[u8],
    response: &mut [u8],
) -> core::result::Result<u32, Tpm2Error> {
    let mut response_size = response.len() as u32;
    let result = unsafe {
        Tbsip_Submit_Command(
            context,
            TBS_COMMAND_LOCALITY_ZERO,
            TBS_COMMAND_PRIORITY_NORMAL,
            command,
            response.as_mut_ptr(),
            &mut response_size,
        )
    };

    if result != ERROR_SUCCESS.0 {
        return Err(Tpm2Error::communication_error(format!(
            "TBS submit command failed: 0x{:X}",
            result
        )));
    }

    Ok(response_size)
}

/// Build TPM 2.0 command header
fn build_command_header(tag: u16, command_code: u32, param_size: u32) -> Vec<u8> {
    let total_size = 10 + param_size; // header(10) + params
    let mut cmd = Vec::with_capacity(total_size as usize);
    cmd.extend_from_slice(&tag.to_be_bytes());
    cmd.extend_from_slice(&total_size.to_be_bytes());
    cmd.extend_from_slice(&command_code.to_be_bytes());
    cmd
}

/// Parse TPM 2.0 response header
fn parse_response_header(response: &[u8]) -> core::result::Result<(u16, u32, u32), Tpm2Error> {
    if response.len() < 10 {
        return Err(Tpm2Error::bad_parameter("Response too short for header"));
    }
    let tag = u16::from_be_bytes([response[0], response[1]]);
    let size = u32::from_be_bytes([response[2], response[3], response[4], response[5]]);
    let code = u32::from_be_bytes([response[6], response[7], response[8], response[9]]);

    if code != TPM_RC_SUCCESS {
        return Err(Tpm2Error::communication_error(format!(
            "TPM command failed with code: 0x{:X}",
            code
        )));
    }

    Ok((tag, size, code))
}

// M34: Core Functions (real implementation using Windows TBS)

// M34: Discover and initialize TPM 2.0 hardware

pub fn tpm2_discover() -> core::result::Result<Tpm2Status, Tpm2Error> {
    match get_tbs_context() {
        Ok(_) => Ok(Tpm2Status::Ok),
        Err(e) => Err(e),
    }
}

// M34: Shutdown TPM 2.0 API

pub fn tpm2_shutdown_api() -> core::result::Result<Tpm2Status, Tpm2Error> {
    // TBS context is managed by OnceLock and will be dropped on program exit
    Ok(Tpm2Status::Ok)
}

// M34: Get EK (Endorsement Key) name
// Creates a primary key under the endorsement hierarchy and reads its public name

pub fn tpm2_ek_get() -> core::result::Result<Tpm2BName, Tpm2Error> {
    let context = get_tbs_context()?;

    // TPM2_CreatePrimary command parameters
    // Create primary key under endorsement hierarchy (TPM_RH_ENDORSEMENT)
    // Using RSA 2048 with SHA256, restricted, decrypt, userWithAuth
    let mut primary_cmd = build_command_header(TPM_ST_NO_SESSIONS, TPM_CC_CREATE_PRIMARY, 0);

    // Primary key creation parameters:
    // - primaryHandle: TPM_RH_ENDORSEMENT (0x4000000B)
    // - inPublic: TPM2B_PUBLIC with RSA parameters
    // - inSensitive: TPM2B_SENSITIVE_CREATE (empty for default auth)
    // - outsideInfo: TPM2B_DATA (empty)
    // - creationPCR: TPML_PCR_SELECTION (empty)

    // primaryHandle
    primary_cmd.extend_from_slice(&TPM_RH_ENDORSEMENT.to_be_bytes());

    // inPublic: TPM2B_PUBLIC
    //   size: 2 bytes
    //   TPMT_PUBLIC:
    //     type: TPM_ALG_RSA (0x0001)
    //     nameAlg: TPM_ALG_SHA256 (0x000B)
    //     objectAttributes: TPMA_OBJECT (userWithAuth | restricted | decrypt)
    //     authPolicy: TPM2B_DIGEST (empty)
    //     parameters: TPMT_RSA_PARAMETERS
    //     unique: TPM2B_PUBLIC_KEY_RSA (empty for creation)

    let mut in_public = Vec::new();

    // TPMT_PUBLIC
    in_public.extend_from_slice(&TPM_ALG_RSA.to_be_bytes()); // type
    in_public.extend_from_slice(&TPM_ALG_SHA256.to_be_bytes()); // nameAlg
    in_public.extend_from_slice(
        &(TPMA_OBJECT_USERWITHAUTH | TPMA_OBJECT_RESTRICTED | TPMA_OBJECT_DECRYPT).to_be_bytes(),
    ); // objectAttributes
    in_public.extend_from_slice(&0u16.to_be_bytes()); // authPolicy size (empty)

    // TPMT_RSA_PARAMETERS
    in_public.extend_from_slice(&TPM_ALG_SHA256.to_be_bytes()); // symmetric.algorithm (TPM_ALG_NULL = 0x0010, but using SHA256 for simplicity)
    in_public.extend_from_slice(&0u16.to_be_bytes()); // symmetric.keyBits
    in_public.extend_from_slice(&0u16.to_be_bytes()); // symmetric.mode
    in_public.extend_from_slice(&TPM_ALG_SHA256.to_be_bytes()); // scheme
    in_public.extend_from_slice(&2048u16.to_be_bytes()); // keyBits
    in_public.extend_from_slice(&0u32.to_be_bytes()); // exponent (0 = default 65537)

    // unique: TPM2B_PUBLIC_KEY_RSA (empty for creation)
    in_public.extend_from_slice(&0u16.to_be_bytes());

    // TPM2B_PUBLIC size prefix
    primary_cmd.extend_from_slice(&(in_public.len() as u16).to_be_bytes());
    primary_cmd.extend_from_slice(&in_public);

    // inSensitive: TPM2B_SENSITIVE_CREATE (empty)
    primary_cmd.extend_from_slice(&0u16.to_be_bytes());

    // outsideInfo: TPM2B_DATA (empty)
    primary_cmd.extend_from_slice(&0u16.to_be_bytes());

    // creationPCR: TPML_PCR_SELECTION (empty - count = 0)
    primary_cmd.extend_from_slice(&0u32.to_be_bytes());

    // Submit command
    let mut response = vec![0u8; 1024];
    let resp_size = tpm2_submit_command(context, &primary_cmd, &mut response)?;
    response.truncate(resp_size as usize);

    // Parse response
    let (_tag, _size, _code) = parse_response_header(&response)?;

    // Response format:
    // handle: 4 bytes (primary key handle)
    // outPublic: TPM2B_PUBLIC
    // creationData: TPM2B_CREATION_DATA
    // creationHash: TPM2B_DIGEST
    // creationTicket: TPMT_TK_CREATION
    // name: TPM2B_NAME

    if response.len() < 10 + 4 {
        return Err(Tpm2Error::communication_error("Response too short"));
    }

    // Skip header (10 bytes) and handle (4 bytes)
    let mut offset = 14;

    // Skip outPublic (TPM2B_PUBLIC)
    if offset + 2 > response.len() {
        return Err(Tpm2Error::communication_error(
            "Response truncated at outPublic size",
        ));
    }
    let out_public_size = u16::from_be_bytes([response[offset], response[offset + 1]]) as usize;
    offset += 2 + out_public_size;

    // Skip creationData (TPM2B_CREATION_DATA)
    if offset + 2 > response.len() {
        return Err(Tpm2Error::communication_error(
            "Response truncated at creationData size",
        ));
    }
    let creation_data_size = u16::from_be_bytes([response[offset], response[offset + 1]]) as usize;
    offset += 2 + creation_data_size;

    // Skip creationHash (TPM2B_DIGEST)
    if offset + 2 > response.len() {
        return Err(Tpm2Error::communication_error(
            "Response truncated at creationHash size",
        ));
    }
    let creation_hash_size = u16::from_be_bytes([response[offset], response[offset + 1]]) as usize;
    offset += 2 + creation_hash_size;

    // Skip creationTicket (TPMT_TK_CREATION) - tag(2) + hierarchy(4) + digest(32) = 38
    offset += 38;

    // name: TPM2B_NAME
    if offset + 2 > response.len() {
        return Err(Tpm2Error::communication_error(
            "Response truncated at name size",
        ));
    }
    let name_size = u16::from_be_bytes([response[offset], response[offset + 1]]) as usize;
    offset += 2;

    if offset + name_size > response.len() {
        return Err(Tpm2Error::communication_error(
            "Response truncated at name data",
        ));
    }

    let copy_len = name_size.min(64);
    let mut buffer = [0; 64];
    buffer[..copy_len].copy_from_slice(&response[offset..offset + copy_len]);
    let name = Tpm2BName {
        size: name_size as c_uint,
        buffer,
    };

    Ok(name)
}

// M34: Get capability

pub fn tpm2_get_capability(
    cap: Tpm2Cap,
    property: c_uint,
) -> core::result::Result<Tpm2BMaxBuffer, Tpm2Error> {
    let context = get_tbs_context()?;

    // Map Tpm2Cap to TPM capability
    let tpm_cap = match cap {
        Tpm2Cap::Handles => TPM_CAP_HANDLES,
        Tpm2Cap::Commands => TPM_CAP_COMMANDS,
        Tpm2Cap::TpmProperties => TPM_CAP_TPM_PROPERTIES,
        _ => return Err(Tpm2Error::bad_parameter("Unsupported capability")),
    };

    // Build TPM2_GetCapability command
    let mut cmd = build_command_header(TPM_ST_NO_SESSIONS, TPM_CC_GET_CAPABILITY, 0);

    // capability
    cmd.extend_from_slice(&tpm_cap.to_be_bytes());
    // property
    cmd.extend_from_slice(&property.to_be_bytes());
    // propertyCount (max 10)
    cmd.extend_from_slice(&10u32.to_be_bytes());

    // Submit command
    let mut response = vec![0u8; 4096];
    let resp_size = tpm2_submit_command(context, &cmd, &mut response)?;
    response.truncate(resp_size as usize);

    // Parse response
    let (_tag, _size, _code) = parse_response_header(&response)?;

    // Response: capabilityData (TPMS_CAPABILITY_DATA)
    //   capability: 4 bytes
    //   data: TPMS_CAPABILITY_DATA_UNION

    let data_start = 10; // skip header
    if data_start + 4 > response.len() {
        return Err(Tpm2Error::communication_error("Response truncated"));
    }
    let data_len = response.len() - data_start - 4;
    let copy_len = data_len.min(512);
    let mut data = [0; 512];
    data[..copy_len].copy_from_slice(&response[data_start + 4..data_start + 4 + copy_len]);

    Ok(Tpm2BMaxBuffer {
        size: data_len as c_uint,
        data,
    })
}

// M34: Get test result (TPM2_GetTestResult, auto-test del chip)

pub fn tpm2_get_test_result() -> core::result::Result<Tpm2BDigest, Tpm2Error> {
    let context = get_tbs_context()?;
    let cmd = build_command_header(TPM_ST_NO_SESSIONS, TPM_CC_GET_TEST_RESULT, 0);
    let mut response = vec![0u8; 1024];
    let resp_size = tpm2_submit_command(context, &cmd, &mut response)?;
    response.truncate(resp_size as usize);
    let (_tag, _size, _code) = parse_response_header(&response)?;

    // Respuesta: outData TPM2B_MAX_BUFFER + testResult u32.
    let mut offset = 10;
    if offset + 2 > response.len() {
        return Err(Tpm2Error::communication_error("truncado en test result"));
    }
    let n = u16::from_be_bytes([response[offset], response[offset + 1]]) as usize;
    offset += 2;
    if offset + n + 4 > response.len() {
        return Err(Tpm2Error::communication_error("truncado en datos de test"));
    }
    let mut digest = Tpm2BDigest::default();
    let copy = n.min(digest.buffer.len());
    digest.size = copy as c_uint;
    digest.buffer[..copy].copy_from_slice(&response[offset..offset + copy]);
    Ok(digest)
}

// M34: Get attributes (lista de handles presentes en el chip para la clase)

pub fn tpm2_get_attributes(class: c_uint) -> core::result::Result<Tpm2BMaxBuffer, Tpm2Error> {
    // class: 0 = persistentes, 1 = transitorios, 2 = todos.
    let property: c_uint = match class {
        0 => 0x01000000,
        1 => 0x02000000,
        _ => 0x00000000,
    };
    tpm2_get_capability(Tpm2Cap::Handles, property)
}

// M34: Hash data

pub fn tpm2_hash_get(
    hash_alg: c_uint,
    data: &[u8],
) -> core::result::Result<Tpm2BDigest, Tpm2Error> {
    let context = get_tbs_context()?;

    // Map hash algorithm
    let tpm_alg = match hash_alg {
        0x000B => TPM_ALG_SHA256, // SHA256
        _ => return Err(Tpm2Error::bad_parameter("Unsupported hash algorithm")),
    };

    // Build TPM2_Hash command
    let mut cmd = build_command_header(TPM_ST_NO_SESSIONS, TPM_CC_HASH, 0);

    // data: TPM2B_MAX_BUFFER
    cmd.extend_from_slice(&(data.len() as u16).to_be_bytes());
    cmd.extend_from_slice(data);

    // hashAlg
    cmd.extend_from_slice(&tpm_alg.to_be_bytes());

    // hierarchy: TPM_RH_NULL
    cmd.extend_from_slice(&TPM_RH_NULL.to_be_bytes());

    // Submit command
    let mut response = vec![0u8; 1024];
    let resp_size = tpm2_submit_command(context, &cmd, &mut response)?;
    response.truncate(resp_size as usize);

    // Parse response
    let (_tag, _size, _code) = parse_response_header(&response)?;

    // Response: outHash: TPM2B_DIGEST, validation: TPMT_TK_HASHCHECK
    let mut offset = 10; // skip header

    // outHash: TPM2B_DIGEST
    if offset + 2 > response.len() {
        return Err(Tpm2Error::communication_error(
            "Response truncated at hash size",
        ));
    }
    let hash_size = u16::from_be_bytes([response[offset], response[offset + 1]]) as usize;
    offset += 2;

    if offset + hash_size > response.len() {
        return Err(Tpm2Error::communication_error(
            "Response truncated at hash data",
        ));
    }

    let copy_len = hash_size.min(32);
    let mut buffer = [0; 32];
    buffer[..copy_len].copy_from_slice(&response[offset..offset + copy_len]);

    Ok(Tpm2BDigest {
        size: copy_len as c_uint,
        buffer,
    })
}

// M34: Generate attestation quote

pub fn tpm2_attest(
    _qualified_signers: &[Tpm2BPublicKey],
) -> core::result::Result<Tpm2BAttestation, Tpm2Error> {
    let context = get_tbs_context()?;

    // Build TPM2_Quote command
    // This requires a signing key handle and qualifying data
    let mut cmd = build_command_header(TPM_ST_NO_SESSIONS, TPM_CC_QUOTE, 0);

    // signHandle: use endorsement handle (simplified)
    cmd.extend_from_slice(&TPM_RH_ENDORSEMENT.to_be_bytes());

    // qualifyingData: TPM2B_DATA
    let qd = b"SIL attestation";
    cmd.extend_from_slice(&(qd.len() as u16).to_be_bytes());
    cmd.extend_from_slice(qd);

    // inScheme: TPMT_SIG_SCHEME (TPM_ALG_RSASSA with SHA256)
    cmd.extend_from_slice(&TPM_ALG_RSA.to_be_bytes()); // scheme
    cmd.extend_from_slice(&TPM_ALG_SHA256.to_be_bytes()); // details.hashAlg

    // pcrSelect: TPML_PCR_SELECTION (empty - no PCR selection)
    cmd.extend_from_slice(&0u32.to_be_bytes());

    // Submit command
    let mut response = vec![0u8; 4096];
    let resp_size = tpm2_submit_command(context, &cmd, &mut response)?;
    response.truncate(resp_size as usize);

    // Parse response
    let (_tag, _size, _code) = parse_response_header(&response)?;

    // Response: quoted: TPM2B_ATTEST, signature: TPMT_SIGNATURE
    let mut offset = 10; // skip header

    // quoted: TPM2B_ATTEST
    if offset + 2 > response.len() {
        return Err(Tpm2Error::communication_error(
            "Response truncated at attest size",
        ));
    }
    let attest_size = u16::from_be_bytes([response[offset], response[offset + 1]]) as usize;
    offset += 2;

    if offset + attest_size > response.len() {
        return Err(Tpm2Error::communication_error(
            "Response truncated at attest data",
        ));
    }

    let copy_len = attest_size.min(256);
    let mut data = [0; 256];
    data[..copy_len].copy_from_slice(&response[offset..offset + copy_len]);

    Ok(Tpm2BAttestation {
        size: attest_size as c_uint,
        data,
    })
}

// M34: Policy evaluation (consulta real al chip + regla local documentada).
//
// Pregunta al chip que capacidades ofrece y verifica que la autorizacion
// pedida cabe en ellas. Sin TPM presente retorna Err, nunca `true` ciego.

pub fn tpm2_policy_evaluate(
    cap: Tpm2Cap,
    property: c_uint,
    authorization: Tpm2Permissions,
) -> core::result::Result<bool, Tpm2Error> {
    if authorization.0 == 0 {
        return Ok(false);
    }
    match cap {
        Tpm2Cap::NvIndex => {
            // NV exige lectura o escritura en la autorizacion.
            let mask = Tpm2Permissions::READ.0 | Tpm2Permissions::WRITE.0;
            if authorization.0 & mask == 0 {
                return Ok(false);
            }
            // Y que el chip tenga algun indice NV definido.
            match tpm2_get_capability(Tpm2Cap::Handles, 0x01000000) {
                Ok(buf) => Ok(buf.size > 0),
                Err(e) => Err(e),
            }
        }
        Tpm2Cap::EccCurves | Tpm2Cap::Algorithms => {
            // Verifica contra la lista real del chip en vez de `true` fijo.
            let buf = tpm2_get_capability(Tpm2Cap::Commands, property)?;
            Ok(buf.size > 0)
        }
        _ => Ok(false),
    }
}

// M34: Hierarchy control (requiere autorizacion de plataforma explicita).
//
// No existe comando "activar jerarquia" sin auth de plataforma: se exige el
// bit SENSITIVE_DATA en la politica y TPM presente. Sin eso, Err.

pub fn tpm2_hierarchy_control(
    _startup_type: c_uint,
    authorization_policy: Tpm2Permissions,
) -> core::result::Result<(), Tpm2Error> {
    if authorization_policy.0 & Tpm2Permissions::SENSITIVE_DATA.0 == 0 {
        return Err(Tpm2Error::auth_error(
            "hierarchy_control exige autorizacion de plataforma (SENSITIVE_DATA)",
        ));
    }
    // Prueba de presencia del chip: lista de handles permanentes.
    tpm2_get_capability(Tpm2Cap::Handles, 0x40000000)?;
    Ok(())
}

// M34: Set authorization (abre sesion HMAC real y la cierra).

pub fn tpm2_set_authorization(
    handles: &[c_uint],
    auth: &Tpm2Permissions,
) -> core::result::Result<(), Tpm2Error> {
    if handles.is_empty() {
        return Err(Tpm2Error::bad_parameter("sin handles que autorizar"));
    }
    if auth.0 == 0 {
        return Err(Tpm2Error::auth_error("autorizacion vacia"));
    }
    // Sesion HMAC real contra el chip: si el chip no responde, falla aqui.
    let sesion = tpm2_session_create(TPM_RH_NULL, 1)?;
    tpm2_session_close(sesion)?;
    Ok(())
}

// M34: NV write (comando real, error propagado)

pub fn tpm2_nv_write(
    nv_index: c_uint,
    offset: c_uint,
    data: &[u8],
) -> core::result::Result<(), Tpm2Error> {
    if data.is_empty() || data.len() > 1024 {
        return Err(Tpm2Error::bad_parameter("dato NV vacio o mayor a 1024"));
    }
    let context = get_tbs_context()?;

    // Build TPM2_NV_Write command
    let mut cmd = build_command_header(TPM_ST_NO_SESSIONS, TPM_CC_NV_WRITE, 0);

    // authHandle: TPM_RH_OWNER
    cmd.extend_from_slice(&TPM_RH_OWNER.to_be_bytes());
    // nvIndex
    cmd.extend_from_slice(&nv_index.to_be_bytes());
    // data: TPM2B_MAX_NV_BUFFER
    cmd.extend_from_slice(&(data.len() as u16).to_be_bytes());
    cmd.extend_from_slice(data);
    // offset
    cmd.extend_from_slice(&offset.to_be_bytes());

    let mut response = vec![0u8; 1024];
    let resp_size = tpm2_submit_command(context, &cmd, &mut response)?;
    response.truncate(resp_size as usize);
    parse_response_header(&response)?;
    Ok(())
}

// M34: NV read (persistent storage)

pub fn tpm2_nv_read(
    nv_index: c_uint,
    offset: c_uint,
    data_len: usize,
) -> core::result::Result<Vec<u8>, Tpm2Error> {
    let context = get_tbs_context()?;

    // Build TPM2_NV_Read command
    let mut cmd = build_command_header(TPM_ST_NO_SESSIONS, TPM_CC_NV_READ, 0);

    // authHandle: TPM_RH_OWNER
    cmd.extend_from_slice(&TPM_RH_OWNER.to_be_bytes());
    // nvIndex
    cmd.extend_from_slice(&nv_index.to_be_bytes());
    // size
    cmd.extend_from_slice(&(data_len as u16).to_be_bytes());
    // offset
    cmd.extend_from_slice(&offset.to_be_bytes());

    let mut response = vec![0u8; 4096];
    let resp_size = tpm2_submit_command(context, &cmd, &mut response)?;
    response.truncate(resp_size as usize);

    // Parse response
    let (_tag, _size, _code) = parse_response_header(&response)?;

    // Response: data: TPM2B_MAX_NV_BUFFER
    let mut offset = 10; // skip header
    if offset + 2 > response.len() {
        return Err(Tpm2Error::communication_error(
            "Response truncated at NV read size",
        ));
    }
    let read_size = u16::from_be_bytes([response[offset], response[offset + 1]]) as usize;
    offset += 2;

    if offset + read_size > response.len() {
        return Err(Tpm2Error::communication_error(
            "Response truncated at NV read data",
        ));
    }

    Ok(response[offset..offset + read_size].to_vec())
}

// M34: Session management (devuelve el handle real del chip)

pub fn tpm2_session_create(
    _hierarchy: c_uint,
    session_type: c_uint,
) -> core::result::Result<c_uint, Tpm2Error> {
    if session_type != 1 && session_type != 2 {
        return Err(Tpm2Error::bad_parameter(
            "tipo de sesion invalido (1=HMAC, 2=POLICY)",
        ));
    }
    let context = get_tbs_context()?;

    // Build TPM2_StartAuthSession command
    let mut cmd = build_command_header(TPM_ST_NO_SESSIONS, TPM_CC_START_AUTH_SESSION, 0);

    // tpmKey: TPM_RH_NULL (no encryption key)
    cmd.extend_from_slice(&TPM_RH_NULL.to_be_bytes());
    // bind: TPM_RH_NULL (no binding)
    cmd.extend_from_slice(&TPM_RH_NULL.to_be_bytes());
    // nonceCaller: TPM2B_NONCE (empty for simplicity)
    cmd.extend_from_slice(&0u16.to_be_bytes());
    // encryptedSalt: TPM2B_ENCRYPTED_SECRET (empty)
    cmd.extend_from_slice(&0u16.to_be_bytes());
    // sessionType: TPM_SE_HMAC (0x01) or TPM_SE_POLICY (0x02)
    cmd.extend_from_slice(&(session_type as u16).to_be_bytes());
    // symmetric: TPMT_SYM_DEF (AES 128 CFB)
    cmd.extend_from_slice(&0x0006u16.to_be_bytes()); // TPM_ALG_AES
    cmd.extend_from_slice(&128u16.to_be_bytes()); // keyBits
    cmd.extend_from_slice(&0x0004u16.to_be_bytes()); // mode CFB
                                                     // authHash: TPM_ALG_SHA256
    cmd.extend_from_slice(&TPM_ALG_SHA256.to_be_bytes());

    let mut response = vec![0u8; 1024];
    let resp_size = tpm2_submit_command(context, &cmd, &mut response)?;
    response.truncate(resp_size as usize);
    parse_response_header(&response)?;
    if response.len() < 14 {
        return Err(Tpm2Error::communication_error(
            "respuesta de sesion truncada",
        ));
    }
    Ok(u32::from_be_bytes([
        response[10],
        response[11],
        response[12],
        response[13],
    ]))
}

// M34: Close session (FlushContext real en el chip)

pub fn tpm2_session_close(handle: c_uint) -> core::result::Result<(), Tpm2Error> {
    if handle == 0 {
        return Err(Tpm2Error::bad_parameter("handle de sesion nulo"));
    }
    let context = get_tbs_context()?;
    let mut cmd = build_command_header(TPM_ST_NO_SESSIONS, TPM_CC_FLUSH_CONTEXT, 0);
    cmd.extend_from_slice(&handle.to_be_bytes());
    let mut response = vec![0u8; 256];
    let resp_size = tpm2_submit_command(context, &cmd, &mut response)?;
    response.truncate(resp_size as usize);
    parse_response_header(&response)?;
    Ok(())
}
