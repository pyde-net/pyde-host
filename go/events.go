//go:build tinygo

// Events (§7.5) — append log entries to the transaction receipt. An event is
// 1..=4 topics plus a borsh-encoded non-indexed data payload. By convention
// topics[0] is Blake3 of the event's canonical signature and any indexed
// fields follow in topics[1:]. Emitting from a view function traps.

package pyde

// EventTopic0 is the canonical topic-0 for an event: Blake3 of its signature
// string, e.g. EventTopic0("Transfer(address,address,u128)").
func EventTopic0(signature string) Bytes32 {
	return Blake3([]byte(signature))
}

// Emit appends an event log. topics must hold 1..=4 entries (topics[0] is the
// signature hash from EventTopic0, indexed fields follow); data is the
// borsh-encoded non-indexed payload.
func Emit(topics []Bytes32, data []byte) {
	n := len(topics)
	if n < 1 || n > 4 {
		Revert("pyde: emit requires 1..=4 topics")
	}
	// The host reads topic_count × 32 contiguous bytes, so flatten first.
	buf := make([]byte, 32*n)
	for i := range topics {
		copy(buf[i*32:], topics[i][:])
	}
	emitEvent(ptr(buf), int32(n), ptr(data), int32(len(data)))
}
