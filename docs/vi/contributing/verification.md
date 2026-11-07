# Kiểm chứng CIRNET Hardware

[English](../../en/contributing/verification.md) · Tiếng Việt

[Mục lục tài liệu](../index.md)

[Quy trình CI](../../../.github/workflows/hardware-service.yml) của kho chạy
trên Ubuntu khi push lên `master`, mở pull request hoặc chạy thủ công. CI dùng
công cụ DEV cùng các wheel Contracts và Simulator đã ghim, dựng wheel và
sdist Python, xác minh wheel đã cài, chạy `tests/` từ thư mục ngoài kho và
tải các gói lên để rà soát. Bản dựng Rust, bộ kiểm thử vòng đời giai đoạn 1
bật riêng và các bộ kiểm thử giai đoạn 3 chạy riêng.

## Kiểm chứng wheel Python đã cài

Dùng Node 26 và Python 3.14 để tái lập môi trường CI hiện tại. Kích hoạt một môi trường Python mới nằm ngoài mọi bản sao kho mã. Chọn manifest Contracts và Simulator 0.1.2 đã rà soát cùng bộ cấp phát DEV đã ghim. Trước khi lưu dữ liệu chẩn đoán, khởi tạo bộ cấp phát và kế thừa phiên cùng ngữ cảnh. Khi kiểm chứng nhiều kho, ngữ cảnh phải liệt kê mọi kho tham gia và chủ sở hữu cụ thể.

Chạy các lệnh PowerShell sau tại kho này. Thay cả hai đường dẫn manifest bằng đường dẫn tuyệt đối đã được rà soát.

Chạy các lệnh theo thứ tự và dừng khi có lỗi đầu tiên.

~~~powershell
$env:CIRNET_CONTRACTS_ARTIFACT_MANIFEST = 'C:/absolute/contracts/artifacts.json'
$env:CIRNET_SIMULATOR_ARTIFACT_MANIFEST = 'C:/absolute/simulator/artifacts.json'
$bootstrap = python -B -c "from scripts.bootstrap_contract_tools import bootstrap; import json; print(json.dumps(bootstrap()))" | Select-Object -Last 1 | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or -not $bootstrap.session -or -not $bootstrap.context) {
    throw 'Could not allocate a verification session.'
}
$env:CIRNET_ARTIFACTS_SESSION = $bootstrap.session
$env:CIRNET_ARTIFACTS_CONTEXT = $bootstrap.context | ConvertTo-Json -Compress -Depth 10
$env:TMP = $env:TEMP = $env:TMPDIR = $bootstrap.session
$env:PIP_CACHE_DIR = Join-Path $bootstrap.session 'pip-cache'
$env:PYTHONDONTWRITEBYTECODE = '1'
$wheelhouse = Join-Path $bootstrap.session 'wheelhouse'
$dist = Join-Path $bootstrap.session 'dist'
python -B -m pip download --require-hashes --only-binary=:all: -r requirements-verification.lock --dest $wheelhouse
python -B -m pip install --require-hashes --only-binary=:all: --no-index --find-links $wheelhouse -r requirements-verification.lock
python -B scripts/install_contracts.py
python -B scripts/install_simulator.py --manifest $env:CIRNET_SIMULATOR_ARTIFACT_MANIFEST
$env:SOURCE_DATE_EPOCH = git log -1 --format=%ct
python -B -m build --no-isolation --outdir $dist
$wheel = Join-Path $dist 'cirnet_hardware_service-0.1.2-py3-none-any.whl'
python -B -m pip install --no-index --no-deps --force-reinstall $wheel
python -B -m pip check
python -B scripts/check_installed.py --wheel $wheel
$tests = Join-Path (Get-Location).Path 'tests'
Push-Location $bootstrap.session
try {
    python -I -B -m pytest $tests --basetemp (Join-Path $bootstrap.session 'pytest')
} finally { Pop-Location }
~~~

Bước kiểm tra wheel đối chiếu tệp đã cài, siêu dữ liệu và RECORD với gói đã chọn. Kiểm thử cách ly chạy RPC qua loopback, kiểm tra NCAP, trạng thái thiếu trình điều khiển vật lý, điều kiện vòng đời và tiến trình con chạy trạm ảo. Trước khi phát hành, kiểm tra có thể dựng từ sdist độc lập với bộ phụ thuộc đã khóa, rồi so sánh từng byte của wheel ở cùng `SOURCE_DATE_EPOCH`. Bản xuất mã nguồn cục bộ và bản dựng từ commit phát hành có nguồn gốc riêng.

## Worker native giai đoạn 1

Dùng môi trường có wheel Hardware và Simulator do manifest ứng viên chọn,
wheel Contracts đã ghim, cùng pytest và psutil từ bộ công cụ kiểm chứng của
Remote. Đặt `CIRNET_NATIVE_STAGE1_CANDIDATE` thành đường dẫn tuyệt đối tới
manifest ứng viên. Chạy lệnh sau từ phiên do mình sở hữu.

~~~powershell
python -I -B -m pytest -q C:/absolute/hardware/tests/native_cpp_lifecycle_cases.py --basetemp "$env:CIRNET_ARTIFACTS_SESSION/hardware-native-lifecycle"
~~~

Bộ kiểm tra đối chiếu mã băm artifact và tệp đã cài, sau đó gửi yêu cầu gRPC
thật qua loopback đến Hardware có tiến trình con C++. Test kiểm soát tiến
trình hoặc thời điểm giao phản hồi để thử Stop lúc hết slot, reset, crash,
timeout và phản hồi đến trong lúc Stop. Mỗi ca xác nhận worker đã thoát,
capture cũ không xuất hiện, phiên phục hồi trả lại đúng byte NCAP và worker
thay thế dừng khi dịch vụ dọn dẹp. Thời gian trong các ca tiêm lỗi được ghi
riêng với số đo hiệu năng.

## Ứng viên Hardware Rust

Dùng bộ công cụ Rust và Cargo `1.99.1` đã ghim để kiểm chứng bản dựng native
hiện tại. Trên Windows, dùng `x86_64-pc-windows-msvc`. Manifest của crate khai
báo Rust `1.90` là phiên bản tối thiểu cho dự án sử dụng crate.
[Phiên bản bộ công cụ](../../../rust-toolchain.toml) · [Manifest của crate](../../../Cargo.toml)

Nếu Rustup chỉ có bí danh `stable`, kiểm tra `rustup run stable rustc -vV`
và `rustup run stable cargo -vV`. Dùng `rustup run stable cargo` cho các
bản dựng hiện tại khi bí danh trỏ tới 1.99.1 trên đúng host.

Bản dựng Windows trong hồ sơ Gate 3 dùng Rust `1.90.0`.
[Bằng chứng Gate 3 trên Windows](../history/stage-3-windows-verification.md)

Chuẩn bị bộ nhớ đệm cho các phụ thuộc Cargo đã khóa bên ngoài bản sao kho
trước khi chạy ngoại tuyến. Bản dựng native dùng `cirnet-contracts-native==0.1.2`
tại commit `e640f80b1199779659d3db7817fee16d13cc24c1`. Đặt
`CIRNET_CONTRACTS_ROOT`, `CIRNET_SIM_CPP_WORKER` và
`CIRNET_SIM_CPP_WORKER_SHA256` theo [cấu hình ứng viên](../guides/getting-started.md#chạy-ứng-viên-hardware-rust).
Chạy từ kho Hardware khi phiên do bộ cấp phát tạo đang hoạt động.

~~~powershell
$env:CARGO_TARGET_DIR = Join-Path $env:CIRNET_ARTIFACTS_SESSION 'hardware-cargo'
cargo build --release --locked --offline
cargo test --locked --offline --lib --bins
cargo test --locked --offline --test simulator_parity -- --ignored
~~~

Bộ kiểm thử đơn vị kiểm tra giao thức truyền, giám sát worker, cách ly kết quả
cũ của trạm, đồng bộ Start và bind loopback. Bốn
[ca kiểm tra tương đương](../../../native/tests/simulator_parity.rs) bật riêng
cần worker đã ghim và tài nguyên Contracts thực. Chúng kiểm tra mẫu số và
NCAP chính xác, preview DC có cấp nguồn và linh kiện chưa hỗ trợ, thứ tự
clock và vector cùng giới hạn sự kiện, và cách gắn linh kiện vật lý từ
`contracts/design-models/fixtures/physical-mounting-parity.json`.

Để chạy bộ kiểm thử RPC loopback riêng, dùng môi trường có pytest, psutil
và các wheel Python Contracts, Hardware, Simulator và Remote đã ghim.
Dùng các wheel tham chiếu giai đoạn 1 và
`CIRNET_NATIVE_STAGE1_CANDIDATE` như trên. Đặt
`CIRNET_NATIVE_STAGE3_HARDWARE` thành đường dẫn tuyệt đối tới binary Rust
và `CIRNET_NATIVE_STAGE3_HARDWARE_SHA256` thành mã băm đã chọn. Nếu wheel
Remote đã cài khác bản giai đoạn 1, đặt `CIRNET_NATIVE_STAGE3_REMOTE_SHA256`
thành mã băm của wheel Remote đã chọn. Giữ `rustc` trong PATH để bộ kiểm thử
dựng worker mô phỏng phản hồi cũ trong thư mục kiểm thử ngoài kho.

~~~powershell
$rpcTests = Join-Path (Get-Location).Path 'native/tests/test_rpc_lifecycle.py'
Push-Location $env:CIRNET_ARTIFACTS_SESSION
try {
    python -I -B -m pytest -q $rpcTests --basetemp (Join-Path $env:CIRNET_ARTIFACTS_SESSION 'hardware-native-rpc')
} finally { Pop-Location }
~~~

[Bộ kiểm thử RPC](../../../native/tests/test_rpc_lifecycle.py) bao quát cả hai
tên RPC, NCAP tương đương, trạm vật lý chưa khả dụng, hủy tác vụ và cách ly
lease, lỗi worker và phản hồi cũ. Các ca QPC chạy trên Windows.
[Bộ chẩn đoán đối sánh](../../../native/tests/test_rpc_stop_diagnostic.py) dùng
cùng các artifact đầu vào và bật riêng qua
`CIRNET_NATIVE_STAGE3_STOP_DIAGNOSTIC=1` hoặc
`CIRNET_NATIVE_STAGE3_RPC_DIAGNOSTIC=1`. Remote phụ trách quy trình Stop
chính thức dưới tải. [Bằng chứng Gate 3 trên Windows](../history/stage-3-windows-verification.md) ·
[Chính sách Gate 3 của Remote](https://github.com/cirnetlab/cirnet-remote/blob/13a35145b6d15644c7be10d95a5410bf69e11867/docs/vi/architecture/native-migration.md#chính-sách-nghiệm-thu-gate-3-theo-môi-trường)

## Kiểm tra và dọn phiên

Ghi kết quả và phạm vi vào Markdown. Kiểm tra bằng chứng thô và xác nhận các tiến trình thuộc phiên đã dừng trước khi chỉ dọn phiên do mình sở hữu. Giữ riêng các bản dựng đã chọn để bàn giao phát hành.

~~~powershell
node scripts/artifact-paths.mjs --cleanup $env:CIRNET_ARTIFACTS_SESSION
~~~

Xem [quy trình CI](../../../.github/workflows/hardware-service.yml) và [hồ sơ kiểm chứng theo ngày](../../en/history/verification-ledger.md). Các bước này xác nhận hành vi phần mềm đã cài trong phạm vi được ghi nhận.
