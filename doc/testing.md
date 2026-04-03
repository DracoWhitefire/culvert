# Testing Strategy

culvert's test suite is built around deterministic register-access tests. All tests run
against in-memory transport implementations; no real hardware is required at any point.

## Test structure

Tests are split between inline unit tests in each `src/client/` module and integration
tests in `tests/scdc.rs`.

### Unit tests (`src/client/`)

Each client module contains tests immediately below the code it covers. They use
`TestTransport`, a 256-byte register array backed transport defined in
`src/client/test_transport.rs`.

`TestTransport` has two constructors:

- `TestTransport::new()` — succeeds on all operations; used for happy-path tests.
- `TestTransport::failing_after(n)` — succeeds for the first `n` operations then returns
  `Err(())`; used to exercise every `?` error branch.

The single generic instantiation (`Scdc<TestTransport>`) is intentional: using one
concrete type avoids LLVM counting per-monomorphisation `?` branches as uncovered, which
would inflate the coverage denominator without corresponding tests.

Each register group has tests covering:

- **Encoding correctness** — every field of a write config maps to the correct bit
  position in the output register(s). One assertion per field, not per struct.
- **Decoding correctness** — every field of a read result is extracted from the correct
  bit position. Register values are crafted to isolate individual bits where necessary.
- **Protocol errors** — invalid enum values returned by the sink produce the correct
  `ProtocolError` variant with the raw register value preserved (e.g. all eleven
  undefined `LtpReq` nibbles 5–15 are each tested individually).
- **Transport error propagation** — every read and write call site has a
  `TestTransport::failing_after(n)` test that triggers failure at that exact operation
  and asserts the error bubbles through as `ScdcError::Transport`.

The `register` module also contains unit tests for the `CedCount` newtype (validity bit
masking, 15-bit value preservation) and `UpdateFlags::new` field ordering.

### Integration tests (`tests/scdc.rs`)

The integration tests use `SimulatedScdc`, a separate infallible transport
(`Error = Infallible`) that exercises the public API through the crate boundary. These
tests confirm that the full round-trip — pre-load registers, call an `Scdc` method,
assert on decoded output or written register state — works correctly when going through
`pub use` re-exports.

Coverage is complementary: unit tests cover all branches and error paths; integration
tests confirm end-to-end bit patterns for each register group with realistic multi-field
values.

### plumbob feature tests (`src/client/plumbob_client.rs`)

When compiled with `--features plumbob`, additional tests exercise the `ScdcClient`
implementation. These call methods through the `plumbob::ScdcClient` trait and assert
that the type conversions between culvert and plumbob's owned types are correct. Error
propagation through the trait boundary is also covered.

## Coverage

CI measures line coverage with `cargo-llvm-cov`. The baseline is stored in
`.coverage-baseline` (currently 100%); CI fails if coverage drops more than 0.1% below
it. New register coverage without tests will trip this.

## Philosophy

`Scdc<T>` runs identically against simulated and real `ScdcTransport` implementations.
A test that cannot run with an in-memory transport does not belong in this repository.
Hardware is never a test dependency.
