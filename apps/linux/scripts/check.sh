#!/usr/bin/env bash
# DiffLoupe Linux 版のコンテナ内検証。
# リポジトリを /work にマウントして fmt / clippy / test / release build / smoke test を実行する。
#
# 使い方:
#   apps/linux/scripts/check.sh            # 全工程(fmt+clippy+test+build+smoke)
#   apps/linux/scripts/check.sh build      # release ビルドのみ
#   apps/linux/scripts/check.sh fmt|clippy|test|smoke|shell
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
IMAGE="diffloupe-linux-dev:ubuntu24.04"
DOCKERFILE="$REPO_ROOT/apps/linux/Dockerfile.dev"

if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
    echo "[check] building image $IMAGE"
    docker build -t "$IMAGE" -f "$DOCKERFILE" "$REPO_ROOT/apps/linux"
fi

# cargo のビルド成果物とレジストリをキャッシュ用ボリュームに逃がす。
# ビルド成果物(CARGO_TARGET_DIR)はホストの core/target や apps/*/target と分けて、
# コンテナ専用の名前付きボリュームに置く(ホストの Mac 向けビルドを Linux 成果物で
# 上書きして差分テストを壊した前例がある)。
DOCKER_RUN=(docker run --rm
    -v "$REPO_ROOT:/work"
    -v diffloupe-cargo-target:/cargo-target
    -e CARGO_TARGET_DIR=/cargo-target
    -v diffloupe-cargo-registry:/opt/cargo/registry
    -v diffloupe-cargo-git:/opt/cargo/git
    -e CARGO_TERM_COLOR=always
    -w /work
    "$IMAGE")

TASK="${1:-all}"

in_container() { "${DOCKER_RUN[@]}" bash -lc "$*"; }

case "$TASK" in
    fmt)
        in_container "cargo fmt --manifest-path core/Cargo.toml --all -- --check && cargo fmt --manifest-path apps/linux/Cargo.toml --all -- --check && cargo fmt --manifest-path apps/edition/Cargo.toml --all -- --check"
        ;;
    clippy)
        in_container "cargo clippy --manifest-path core/Cargo.toml --all-targets -- -D warnings && cargo clippy --manifest-path apps/linux/Cargo.toml --all-targets -- -D warnings"
        ;;
    test)
        in_container "cargo test --manifest-path core/Cargo.toml && cargo test --manifest-path apps/linux/Cargo.toml"
        ;;
    build)
        in_container "cargo build --release --manifest-path apps/linux/Cargo.toml"
        ;;
    smoke)
        in_container "apps/linux/scripts/smoke_in_container.sh"
        ;;
    shell)
        "${DOCKER_RUN[@]}" -it bash
        ;;
    all)
        in_container "cargo fmt --manifest-path core/Cargo.toml --all -- --check \
            && cargo fmt --manifest-path apps/linux/Cargo.toml --all -- --check \
            && cargo fmt --manifest-path apps/edition/Cargo.toml --all -- --check \
            && cargo clippy --manifest-path core/Cargo.toml --all-targets -- -D warnings \
            && cargo clippy --manifest-path apps/linux/Cargo.toml --all-targets -- -D warnings \
            && cargo test --manifest-path core/Cargo.toml \
            && cargo test --manifest-path apps/linux/Cargo.toml \
            && cargo build --release --manifest-path apps/linux/Cargo.toml \
            && apps/linux/scripts/smoke_in_container.sh"
        ;;
    *)
        echo "usage: $0 [fmt|clippy|test|build|smoke|shell|all]" >&2
        exit 2
        ;;
esac
