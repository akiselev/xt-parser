#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if (($# == 0)); then
    include_synthetic=true
    manifests=(
        inputs/oracle-binary-abcd-25/manifest.json
        inputs/oracle-binary-abcd-250/manifest.json
    )
else
    include_synthetic=false
    manifests=("$@")
fi

host_path() {
    local path="$1"
    if [[ "$path" == Z:\\* ]]; then
        path="${path#Z:\\}"
        path="/${path//\\//}"
    fi
    printf '%s\n' "$path"
}

cargo build -p xt-schema --bin xt-format-compare

families_passed=0
families_failed=0
nodes_per_encoding=0

for manifest in "${manifests[@]}"; do
    while IFS=$'\t' read -r text bare neutral typed; do
        text="$(host_path "$text")"
        bare="$(host_path "$bare")"
        neutral="$(host_path "$neutral")"
        typed="$(host_path "$typed")"
        if report="$(
            ./target/debug/xt-format-compare \
                "$text" "$bare" "$neutral" "$typed" 2>&1
        )"; then
            families_passed=$((families_passed + 1))
            node_count="$(jq -r '.node_count' <<<"$report")"
            nodes_per_encoding=$((nodes_per_encoding + node_count))
        else
            families_failed=$((families_failed + 1))
            printf 'FAIL %s\n%s\n' "$text" "$report" >&2
        fi
    done < <(
        jq -r '
            .fixtures[]
            | [
                .input,
                (.outputs[] | select(.format == "bare") | .path),
                (.outputs[] | select(.format == "neutral") | .path),
                (.outputs[] | select(.format == "typed") | .path)
            ]
            | @tsv
        ' "$manifest"
    )
done

if "$include_synthetic"; then
    for name in block cube sphere cyl cone torus; do
        if report="$(
            ./target/debug/xt-format-compare \
                "inputs/oracle-binary/${name}_text.xmt_txt" \
                "inputs/oracle-binary/${name}_bare.xmt_bin" \
                "inputs/oracle-binary/${name}_neutral.xmt_bin" \
                "inputs/oracle-binary/${name}_typed.xmt_bin" 2>&1
        )"; then
            families_passed=$((families_passed + 1))
            node_count="$(jq -r '.node_count' <<<"$report")"
            nodes_per_encoding=$((nodes_per_encoding + node_count))
        else
            families_failed=$((families_failed + 1))
            printf 'FAIL synthetic/%s\n%s\n' "$name" "$report" >&2
        fi
    done
fi

printf \
    'families_passed=%d families_failed=%d binary_files=%d binary_node_instances=%d\n' \
    "$families_passed" \
    "$families_failed" \
    "$((families_passed * 3))" \
    "$((nodes_per_encoding * 3))"

test "$families_failed" -eq 0
