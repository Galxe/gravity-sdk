#!/bin/bash

# --- 配置 ---
export WORKSPACE=$(pwd)
bin_name="reth"
chain="mainnet"
JSON_FILE="test.json"
LOG_FILE="${WORKSPACE}/logs/debug.log"

# --- 脚本主体 ---
set -e

# 1. Check if jq is installed
check_jq_installed() {
    if ! command -v jq &> /dev/null; then
        echo "Error: 'jq' is required but not installed. Please install 'jq' first."
        exit 1
    fi
}

check_jq_installed

# 2. Check if JSON config file exists
check_json_file_exists() {
    local json_file="$1"
    if [ ! -f "$json_file" ]; then
        echo "Error: JSON config file '$json_file' not found."
        exit 1
    fi
}

check_json_file_exists "$JSON_FILE"

# 3. Parse JSON and build argument array
parse_json_args() {
    local json_file="$1"
    json_args=()
    while IFS= read -r key && IFS= read -r value; do
        if [ -z "$value" ] || [ "$value" == "null" ]; then
            json_args+=( "--${key}" )
        else
            json_args+=( "--${key}" "${value}" )
        fi
    done < <(jq -r '. | to_entries[] | .key, .value' "$json_file")
}

echo "Parsing arguments from $JSON_FILE ..."
parse_json_args "$JSON_FILE"

# 4. 构建最终的命令
# 注意：我们把之前硬编码的 --dev 和 --http.disable_compression 移除了，因为它们现在由JSON文件控制
cmd=(
    "${WORKSPACE}/bin/${bin_name}"
    "node"
    "--chain" "${chain}"
    "--http"
    # 使用 "${json_args[@]}" 安全地展开数组
    "${json_args[@]}"
)

# 5. 执行命令
echo "准备执行以下命令:"
printf "%q " "${cmd[@]}"
echo
