#!/usr/bin/env bash
set -euo pipefail

schema_source_dir="${PARASOLID_SCHEMA_DIR:-/mnt/c/Program Files/SOLIDWORKS Corp/SOLIDWORKS/data/pschema}"
check=false
if [[ "${1:-}" == "--check" ]]; then
    check=true
fi

generate() {
    local schema_name="$1"
    local output_path="crates/xt-schema/src/schemas/${schema_name}"

    if [[ "$check" == false ]]; then
        cargo run --quiet -p xt-schema-gen -- \
            "$schema_source_dir/${schema_name}.s_t" \
            "$output_path"
        find "$output_path" -type f -name '*.rs' -print0 \
            | xargs -0 rustfmt --edition 2024
        return
    fi

    local temporary_dir
    temporary_dir="$(mktemp -d)"
    trap 'rm -rf -- "$temporary_dir"' RETURN
    local temporary_path="$temporary_dir/${schema_name}"
    cargo run --quiet -p xt-schema-gen -- \
        "$schema_source_dir/${schema_name}.s_t" \
        "$temporary_path"
    find "$temporary_path" -type f -name '*.rs' -print0 \
        | xargs -0 rustfmt --edition 2024
    diff --recursive --brief "$temporary_path" "$output_path" >/dev/null || {
        echo "$output_path is stale; run scripts/generate-schemas.sh" >&2
        return 1
    }
}

generate sch_13006
generate sch_30100
generate sch_37102
