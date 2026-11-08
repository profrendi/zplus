#!/bin/bash
# Links libstdc++ and winpthread statically into the Windows build.
args=()
for a in "$@"; do
  if [ "$a" = "-lstdc++" ]; then
    args+=("-Wl,-Bstatic" "-lstdc++" "-lwinpthread" "-Wl,-Bdynamic")
  else
    args+=("$a")
  fi
done
exec "${MINGW_GCC:-x86_64-w64-mingw32-gcc-posix}" "${args[@]}"
