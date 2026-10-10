// M34: TPM 2.0 API Bindings
// Declaraciones puretas para TPM 2.0 capabilities.
// Implementación real: Windows TPM SAPI, Linux tpm2-cli, o TPM hardware real.
// Este archivo provee la arquitectura completa lista para cuando haya TPM 2.0 physical.

#ifndef SIL_TPM2_H
#define SIL_TPM2_H

#include <stdint.h>
#include <stddef.h>

// M34: TPM 2.0 Error Codes
typedef enum {
    TPM2_RC_SUCCESS = 0,
    TPM2_RC_FAILURE,
    TPM2_RC_AUTHORIZATION_INVALID,
    TPM2_RC_HANDLE_INVALID,
    TPM2_RC_INPUT_SIZE,
    TPM2_RC_BAD_PARAMETER,
    TPM2_RC_RATE_LIMIT,
    TPM2_RC_NV_SPACE,
    TPM2_RC_NV_LOCKED,
    TPM2_RC_NV_DISABLED,
    TPM2_RC_NV_RATE,
    TPM2_RC_NV_AUTHORIZATION,
    TPM2_RC_NV_UNAVAILABLE,
    TPM2_RC_MEMORY,
    TPM2_RC_NEGATIVE_RATE,
    TPM2_RC_PO,
    TPM2_RC_NV_RANGE,
    TPM2_RC_NV_AUTHORIZATION_COUNTER,
    TPM2_RC_NV_RATE_EXCEEDED,
    TPM2_RC_NV_MAX
} TPM2_RC;

// M34: TPM 2.0 Handles
typedef uint32_t TPM2H_HANDLE;

// M34: TPM 2.0 Basic Types
typedef uint8_t  TPM2B_MAX_SIZE[512];
typedef size_t   TPM2B_MAX_SIZE_LENGTH;

// M34: Nonce structures
typedef struct {
    uint8_t odd[32];
    uint8_t even[32];
    size_t size;
} TPM2B_NONCE;

// M34: Authorization Data
typedef struct {
    uint8_t buffer[32];
    size_t size;
} TPM2B_AUTH;

// M34: Public Key Structures
typedef struct {
    uint8_t x[64];
    uint8_t y[64];
    size_t size;
} TPM2B_PUBLIC_KEY_RSA;

typedef struct {
    uint8_t x[32];
    uint8_t y[32];
    size_t size;
} TPM2B_PUBLIC_KEY_EC;

// M34: Capability Identifiers
typedef enum {
    TPM2_CAP_HANDLES = 0x00,
    TPM2_CAP_COMMANDS = 0x01,
    TPM2_CAP_AUDIT_STATES = 0x02,
    TPM2_CAP_PP_COMMANDS = 0x03,
    TPM2_CAP_TPM_PROPERTIES = 0x04,
    TPM2_CAP_TOTAL_COMMANDS = 0x05,
    TPM2_CAP_SESSIONS = 0x06,
    TPM2_CAP_COMMAND_CODE = 0x07,
    TPM2_CAP_NV_INDEX = 0x08,
    TPM2_CAP_ECC_CURVES = 0x09,
    TPM2_CAP_ALGORITHMS = 0x0A,
    TPM2_CAP_SYMMETRIC_ENCRYPT = 0x0B,
    TPM2_CAP_PUBLIC = 0x0C,
    TPM2_CAP_OBJECT_TABLE = 0x0D,
    TPM2_CAP_TIME_AUTHORIZATION = 0x0E,
    TPM2_CAP_ECC_MODES = 0x0F,
    TPM2_CAP_FIPS = 0x10,
    TPM2_CAP_REBOOT_RESET = 0x11,
    TPM2_CAP_PERSISTENT = 0x12,
    TPM2_CAP_AUTH_HANDLES = 0x13,
    TPM2_CAP_PLATFORM = 0x14,
    TPM2_CAP_TOTAL_NV = 0x15,
    TPM2_CAP_PLATFORM_CONFIG = 0x16,
    TPM2_CAP_RESET_RESET = 0x17,
    TPM2_CAP_CONTRACT = 0x18,
    TPM2_CAP_MAX
} TPM2_CAP;

// M34: TPM 2.0 Property Identifiers
typedef enum {
    TPM2_PT_MANUFACTURER = 0x0001,
    TPM2_PT_FAMILY = 0x0002,
    TPM2_PT_VERSION = 0x0003,
    TPM2_PT_MANUFACTURER_STRING = 0x0004,
    TPM2_PT_MODEL = 0x0005,
    TPM2_PT_DESCRIPTION = 0x0006,
    TPM2_PT_UTF8 = 0x0007,
    TPM2_PT_I2C = 0x0008,
    TPM2_PT_SPI = 0x0009,
    TPM2_PT_PCI = 0x000A,
    TPM2_PT_OSAP_CAPABILITIES = 0x000F,
    TPM2_PT_POLICY_PROTOCOL = 0x0010,
    TPM2_PT_SPECIALIZATION = 0x0011,
    TPM2_PT_PRESENCE = 0x0012,
    TPM2_PT_PROTOCOL_NUMBERS = 0x0013,
    TPM2_PT_FIPS = 0x0014,
    TPM2_PT_NUM
} TPM2_PT;

// M34: TPM 2.0 Algorithm Identifiers
typedef enum {
    TPM2_ALG_NULL = 0x00000000,
    TPM2_ALG_RSA = 0x00000001,
    TPM2_ALG_ECC = 0x00000002,
    TPM2_ALG_SYMCIPHER = 0x00000010,
    TPM2_ALG_HASH = 0x00000020,
    TPM2_ALG_KDF = 0x00000040,
    TPM2_ALG_MAC = 0x00000080,
    TPM2_ALG_RSASSA = 0x00010000,
    TPM2_ALG_RSAPSS = 0x00010001,
    TPM2_ALG_ECDSA = 0x00010002,
    TPM2_ALG_ECDAA = 0x00010003,
    TPM2_ALG_SHA256 = 0x0000000B,
    TPM2_ALG_SHA384 = 0x0000000C,
    TPM2_ALG_SHA1 = 0x00000004,
    TPM2_ALG_MAX
} TPM2_ALG;

// M34: TPM 2.0 Permission Types
typedef uint32_t TPM2_PERMISSIONS;
#define TPM2_PERM_SENSITIVE_DATA 0x00000001
#define TPM2_PERM_EXTEND         0x00000002
#define TPM2_PERM_CLEAR          0x00000004
#define TPM2_PERM_WRITE          0x00000008
#define TPM2_PERM_READ           0x00000010
#define TPM2_PERM_CMK            0x00000020
#define TPM2_PERM_EK             0x00000040
#define TPM2_PERM_TPML           0x00000080

// M34: TPM 2.0 Object Attributes
typedef uint16_t TPM2OA_OBJECT_ATTRIBUTES;
#define TPM2OA_USER_WITH_AUTH 0x0001
#define TPM2OA_RESTRICTED     0x0002
#define TPM2OA_DECRYPT        0x0004
#define TPM2OA_USER           0x0008
#define TPM2OA_SIGN_ENCRYPT   0x0010
#define TPM2OA_FIXED          0x0020
#define TPM2OA_FIXED_POSITION 0x0040
#define TPM2OA_SENSITIVE      0x0080
#define TPM2OA_SENSITIVE_RESET 0x0100
#define TPM2OA_EXTEND         0x0200
#define TPM2OA_NODA           0x0400
#define TPM2OA_RESTRICT       0x0800
#define TPM2OA_WRAPPED          0x1000
#define TPM2OA_CHARGED          0x2000

// M34: TPM 2.0 Handle Types
typedef enum {
    TPM_HT_PERSISTENT = 0x00000001,
    TPM_HT_TRANSIENT = 0x00000002,
    TPM_HT_PERMANENT = 0x00000003,
    TPM_HT_CAP_HANDLES = 0x00000004,
    TPM_HT_OBJECT_SESSION = 0x00000005,
    TPM_HT_SESSION = 0x00000005,
    TPM_HT_MAX
} TPM2_HT;

// M34: TPM 2.0 Session Types
typedef enum {
    TPM_SE_NON = 0x00000000,
    TPM_SE_OWNER = 0x00000001,
    TPM_SE_PLATFORM = 0x00000002,
    TPM_SE_MAX
} TPM2_SE;

// M34: TPM 2.0 Command Codes (selectors)
typedef uint32_t TPM2_COMMAND_CODE;

// M34: TPM 2.0 Startup Type
typedef enum {
    TPM2_SU_CLEAR = 0x00000000,
    TPM2_SU_STATE = 0x00000001,
    TPM2_SU_RESET = 0x00000002,
    TPM2_SU_MAX
} TPM2_SU;

// M34: TPM 2.0 Power State
typedef uint32_t TPM2_POWER_STATE;

// M34: Main TPM 2.0 Function Declarations (pure / to be implemented per platform)

// M34: Discovery and Initialization
TPM2_RC tpm2_self_test(void);
TPM2_RC tpm2_get_capability(TPM2_CAP cap, uint32_t property,
                           const TPM2B_AUTH *in_auth,
                           TPM2B_MAX_BUFFER *out_data,
                           uint32_t *out_data_size);
TPM2_RC tpm2_get_test_result(TPM2B_DIGEST *out_digest);
TPM2_RC tpm2_get_attributes(TPMI_CAP_ATTRIBUTES class,
                           TPM2B_AUTH *in_auth,
                           TPM2B_CAP_ATTRIBUTES *out_data);

// M34: Key Hierarchy Management
TPM2_RC tpm2_ek_get(TPM2B_NAME *out_name);
TPM2_RC tpm2_hierarchy_control(TPMI_RH_HIERARCHY handle,
                              TPM2_SU startupType,
                              TPM2_PERMISSIONS authorizationPolicy);
TPM2_RC tpm2_hash_get(TPMI_ALG_HASH hash,
                     const uint8_t *data,
                     size_t data_size,
                     TPM2B_DIGEST *out_digest);

// M34: Attestation Primitives
TPM2_RC tpm2_attest_get_capability(TPMI_RH_PROVISION handle,
                                   TPM2B_NAME *qualified_signers,
                                   TPM2B_MAX_BUFFER *out_data,
                                   uint32_t *out_data_size);
TPM2_RC tpm2_attest_get_statistics(TPMI_RH_PROVISION handle,
                                  TPM2B_MAX_BUFFER *out_data,
                                  uint32_t *out_data_size);

// M34: Policy Management
TPM2_RC tpm2_policy_nv_read(TPMI_RH_NV_INDEX nvIndex,
                             uint16_t offset,
                             size_t data_size,
                             TPM2B_MAX_BUFFER *out_data);
TPM2_RC tpm2_policy_get(TPMI_RH_HANDLE handles[],
                       uint32_t count,
                       TPM2B_AUTH *out_data,
                       uint32_t *out_data_size);

// M34: NV Index Management
TPM2_RC tpm2_nv_write(TPMI_RH_NV_INDEX nvIndex,
                       uint16_t offset,
                       size_t data_size,
                       const uint8_t *data);
TPM2_RC tpm2_nv_read(TPMI_RH_NV_INDEX nvIndex,
                      uint16_t offset,
                      size_t data_size,
                      TPM2B_MAX_BUFFER *out_data);

// M34: Session Management
TPM2_RC tpm2_session_create(TPMI_RH_HANDLE hierarchy,
                           TPM2_SE sessionType,
                           TPM2B_AUTH *bindAuth,
                           TPM2B_AUTH *hmacSecret,
                           TPM2_RC *out_handle);
TPM2_RC tpm2_session_close(TPMI_RH_HANDLE handle);

// M34: Object Operations
TPM2_RC tpm2_create(TPMI_RH_HANDLE parentHandle,
                   TPM2OA_OBJECT_ATTRIBUTES objectAttributes,
                   TPM2B_PUBLIC_KEY_RSA *outPublic,
                   TPM2B_NAME *outName,
                   TPM2B_DIGEST *outQualifyingData,
                   TPMI_RH_HANDLE *outHandle);
TPM2_RC tpm2_flush_context(TPMI_RH_HANDLE handle);

// M34: Utility Functions
TPM2_RC tpm2_init(void);
TPM2_RC tpm2_shutdown(void);
TPM2_RC tpm2_get_time(uint64_t *out_time);

// M34: Error String (for debugging)
const char* tpm2_rc_to_string(TPM2_RC rc);

#endif // SIL_TPM2_H