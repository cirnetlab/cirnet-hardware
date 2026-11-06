# Kiểm chứng wheel Hardware

[English](../en/verification.md) · Tiếng Việt

[Mục lục tài liệu](index.md)

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

Ghi kết quả và phạm vi vào Markdown. Kiểm tra bằng chứng thô và xác nhận các tiến trình thuộc phiên đã dừng trước khi chỉ dọn phiên do mình sở hữu. Giữ riêng các bản dựng đã chọn để bàn giao phát hành.

~~~powershell
node scripts/artifact-paths.mjs --cleanup $env:CIRNET_ARTIFACTS_SESSION
~~~

Xem [quy trình CI](../../.github/workflows/hardware-service.yml) và [hồ sơ kiểm chứng theo ngày](../VERIFICATION.md). Các bước này xác nhận hành vi phần mềm đã cài trong phạm vi được ghi nhận.
