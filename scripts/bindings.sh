#!/usr/bin/env bash
# Generate the Swift and Kotlin bindings for boulevard-core with UniFFI.
# Output: apps/ios/Generated/ (Swift + modulemap) and apps/android/.../generated/ (Kotlin).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p boulevard-core --features ffi --release
case "$(uname -s)" in
  Darwin) LIB=target/release/libboulevard_core.dylib ;;
  *)      LIB=target/release/libboulevard_core.so ;;
esac
BINDGEN=(cargo run -q -p boulevard-core --features ffi --bin uniffi-bindgen --)
"${BINDGEN[@]}" generate --library "$LIB" --language swift  --out-dir apps/ios/Generated
"${BINDGEN[@]}" generate --library "$LIB" --language kotlin --out-dir apps/android/app/src/main/java
echo "Swift:  $(ls apps/ios/Generated)"
echo "Kotlin: $(find apps/android/app/src/main/java -name '*.kt' | head -3)"
