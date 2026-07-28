// ABI status / error codes (HOST_FN_ABI_SPEC §4). Host fns that return an
// i32 use 0 for success and these negative sentinels for failure. Kept
// untagged (no build constraint) so off-chain tooling can name them too.

package pyde

const (
	// StatusOK is the success return of every i32-returning host fn.
	StatusOK int32 = 0

	// SloadMissing is returned by sload/sload_* when a slot was never written.
	SloadMissing int32 = -1

	// §4 core error codes.
	ErrInsufficientBalance     int32 = -3
	ErrReentrancyBlocked       int32 = -9
	ErrCrossCallFailed         int32 = -10 // sub-call trapped or reverted
	ErrValueTransferNotPayable int32 = -12
	ErrInvalidFunctionName     int32 = -13

	// Typed-storage errors (schema / type validation).
	ErrSchemaMissing     int32 = -30
	ErrUnknownField      int32 = -31
	ErrFieldKindMismatch int32 = -32
	ErrKeyTypeMismatch   int32 = -33
	ErrValueTypeMismatch int32 = -34

	// Factory (instantiate) errors.
	ErrCtorReverted        int32 = -40
	ErrTemplateNotContract int32 = -43
	ErrChildAddressTaken   int32 = -44
	ErrInitOnCtorless      int32 = -45
	ErrPrefixCollision     int32 = -46
	ErrPerTxCapReached     int32 = -48
)
