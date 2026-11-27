# Hardware version 0 changelog · Nhật ký thay đổi phiên bản 0

[Changelog index · Chỉ mục](README.md)

## Unreleased

### Changed

- Consolidated English and Vietnamese development history in this version file.
- Hợp nhất lịch sử phát triển tiếng Anh và tiếng Việt trong tệp theo phiên bản.

## v0.3.0-dev.1 — Development

This is a development version used in the Windows x64 Native Candidate workflow. The available Git tags and GitHub Releases provide no publication date for this version.

Đây là phiên bản phát triển dùng trong quy trình Windows x64 Native Candidate. Các Git tag và GitHub Release hiện có không cung cấp ngày phát hành cho phiên bản này.

### Changed

- Co-locate private RPC field projections with the handlers and document the service, station and Simulator module ownership.
  - VI · Đặt phần chuyển đổi trường RPC nội bộ cùng trình xử lý và ghi rõ trách nhiệm của các mô-đun dịch vụ, trạm và Simulator.
- Align repository references with the canonical CIRNET names.
  - VI · Đồng bộ tham chiếu với tên kho CIRNET chính thức.
- Migrate the native Rust service, private Preview definition and tests into conventional Cargo paths.
  - VI · Chuyển dịch vụ Rust native, định nghĩa Preview nội bộ và các bài kiểm thử sang bố cục Cargo chuẩn.
- Build against Contracts Next and verify the Simulator Next worker digest at the runtime boundary.
  - VI · Dựng cùng Contracts Next và xác minh mã băm worker Simulator Next tại ranh giới thực thi.
- Preserve local Hardware RPC 2.0, station lifecycle and protected Stop behavior.
  - VI · Giữ Hardware RPC 2.0 cục bộ, vòng đời trạm và hành vi Stop có bảo vệ.
- Reference CIRNET Lab-First for the station safety boundary.
  - VI · Tham chiếu CIRNET Lab-First cho ranh giới an toàn trạm.
- Run Windows component CI on master pushes with pinned private dependencies.
  - VI · Chạy CI thành phần Windows khi push lên master với phụ thuộc riêng tư đã ghim.
- Report the exact private dependency permissions required by Hardware CI.
  - VI · Nêu quyền cần thiết trên từng repo private mà Hardware CI đọc.
- Verify plan and capture isolation, malformed NCAP evidence and worker digest rejection.
  - VI · Kiểm chứng cách ly kế hoạch và dữ liệu thu, từ chối dữ liệu NCAP không hợp lệ và mã băm worker sai.
- Verify native Hardware RPC with the selected Contracts schema, checked-in reference replies and C++ worker.
  - VI · Kiểm chứng RPC Hardware native bằng schema Contracts đã chọn, phản hồi tham chiếu đã lưu trong kho và worker C++.
- Documented a single owned verification session for Cargo, generated RPC bindings, fixtures and pytest output.
  - VI · Ghi hướng dẫn dùng một phiên kiểm chứng có chủ sở hữu cho Cargo, binding RPC sinh tự động, fixture và đầu ra pytest.

### Fixed

- Compile the stale-generation RPC test worker before pytest on Windows CI.
  - VI · Biên dịch worker kiểm thử generation cũ trước khi chạy pytest trên CI Windows.

### Sources · Nguồn đối chiếu

- [Version manifest · Manifest phiên bản](../../Cargo.toml)
- [Native implementation baseline · Mốc triển khai native](https://github.com/cirnetlab/cirnet-hardware/commit/2288cc44233b537752fb40d251e7b01d51004337)
- [Latest source milestone recorded here · Mốc mã nguồn mới nhất ghi ở đây](https://github.com/cirnetlab/cirnet-hardware/commit/15056907c680989223daafc964a5500be5fa4eaa)
- [Earlier version index placeholder · Tệp chỉ mục phiên bản trước đây](https://github.com/cirnetlab/cirnet-hardware/blob/13a5888de6bc35a52f1117768483024e8a1b114a/docs/changelogs/CHANGELOG_V0.md)
