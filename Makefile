.PHONY: build release debug run clean install uninstall

# 기본 타겟
all: build

# 릴리스 빌드
build:
	cargo build --release

# 디버그 빌드
debug:
	cargo build

# 릴리스 빌드 후 실행
run: build
	./target/release/pdl

# 디버그 모드로 실행
run-debug: debug
	./target/debug/pdl

# 빌드 산출물 정리
clean:
	cargo clean

# 로컬에 설치 (~/.cargo/bin)
install: build
	cargo install --path .

# 설치 제거
uninstall:
	cargo uninstall pdl

# 의존성 확인
check:
	cargo check

# 테스트 실행
test:
	cargo test

# 포맷팅
fmt:
	cargo fmt

# 린트
lint:
	cargo clippy
