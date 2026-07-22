// Host-side conformance test for <pyde/child.h> against the shared
// golden vectors (vectors/child_address.json, via the generated
// vectors_gen.h). Recomputes every 107-byte preimage byte-for-byte
// and pins the unordered-pair salt-source encoding — including that
// the recorded pair args stay DELIBERATELY unsorted, so a naive
// concat differs and the bytewise sort is actually exercised.
// Poseidon2 itself is on-chain-only (hash_poseidon2 host fn) and is
// covered by the engine/binding suites, not here.
//
// Build: cc -std=c11 -Wall -Wextra -Werror -Ic/include \
//          -o child_test c/tests/child_test.c && ./child_test

#include <stdio.h>
#include <string.h>

#include <pyde/child.h>

#include "vectors_gen.h"

_Static_assert(PYDE_CHILD_PREIMAGE_LEN == 107,
               "preimage is 11 (prefix) + 3 * 32 bytes");
_Static_assert(PYDE_CHILD_PREFIX_LEN == 11,
               "\"pyde-child:\" is 11 ASCII bytes");

static int failures = 0;

static void check(int ok, const char* name, const char* what)
{
    if (!ok) {
        fprintf(stderr, "FAIL %s: %s\n", name, what);
        failures++;
    }
}

int main(void)
{
    const size_t n = PYDE_CHILD_VECTOR_COUNT;
    int pair_vectors = 0;

    check(n > 0, "(suite)", "no vectors — vectors_gen.h stale or empty");

    for (size_t i = 0; i < n; i++) {
        const pyde_child_vector* v = &PYDE_CHILD_VECTORS[i];

        uint8_t preimage[PYDE_CHILD_PREIMAGE_LEN];
        pyde_child_preimage(v->parent, v->template_addr, v->salt, preimage);
        check(memcmp(preimage, v->preimage, PYDE_CHILD_PREIMAGE_LEN) == 0,
              v->name, "recomputed preimage != recorded preimage");

        if (!v->has_pair_args)
            continue;
        pair_vectors++;

        check(v->pair_sorted_len == 64, v->name,
              "pair vector without a 64-byte salt_source_borsh");

        // The vectors record the args unsorted ON PURPOSE; if this
        // fires, the vectors file changed and the sort is no longer
        // exercised.
        check(memcmp(v->pair_a, v->pair_b, 32) > 0, v->name,
              "recorded pair args are no longer unsorted");

        uint8_t enc[64];
        pyde_unordered_pair_encoding(v->pair_a, v->pair_b, enc);
        check(memcmp(enc, v->pair_sorted, 64) == 0, v->name,
              "sorted pair encoding != recorded salt_source_borsh");

        uint8_t enc_rev[64];
        pyde_unordered_pair_encoding(v->pair_b, v->pair_a, enc_rev);
        check(memcmp(enc, enc_rev, 64) == 0, v->name,
              "pair encoding is not argument-order independent");

        uint8_t naive[64];
        memcpy(naive, v->pair_a, 32);
        memcpy(naive + 32, v->pair_b, 32);
        check(memcmp(naive, v->pair_sorted, 64) != 0, v->name,
              "naive unsorted concat equals sorted encoding");
    }

    if (failures > 0) {
        fprintf(stderr, "child_test: %d check(s) FAILED across %zu vectors\n",
                failures, n);
        return 1;
    }
    printf("child_test: %zu/%zu vectors OK (%d pair)\n", n, n, pair_vectors);
    return 0;
}
