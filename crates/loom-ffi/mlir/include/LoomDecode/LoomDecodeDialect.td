#ifndef LOOM_DECODE_DIALECT_TD
#define LOOM_DECODE_DIALECT_TD

include "mlir/IR/OpBase.td"

def LoomDecode_Dialect : Dialect {
  let name = "loom.decode";
  let cppNamespace = "::loom::decode";
  let summary = "Verifier-gated Loom decode dialect";
  let description = [{
    The `loom.decode` dialect is an internal post-verification backend surface.
    It is generated only from accepted Loom artifact facts and Phase 22 runtime
    native-candidate plans.

    Verifier hooks on these ops check operation shape, required attributes, and
    region structure. They are not a substitute for Loom artifact verification,
    solver discharge, runtime planning, cache-key validation, or backend policy.
  }];
}

#endif // LOOM_DECODE_DIALECT_TD
