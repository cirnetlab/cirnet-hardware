# Hardware changelog

## 0.3.0-dev.1

- Co-locate private RPC field projections with the handlers and document the service, station and Simulator module ownership.
- Align repository references with the canonical CIRNET names.
- Migrate the native Rust service, private Preview definition and tests into conventional Cargo paths.
- Build against Contracts Next and verify the Simulator Next worker digest at the runtime boundary.
- Preserve local Hardware RPC 2.0, station lifecycle and protected Stop behavior.
- Reference CIRNET Lab-First for the station safety boundary.
- Run Windows component CI on master pushes with pinned private dependencies.
- Report the exact private dependency permissions required by Hardware CI.
- Verify plan and capture isolation, malformed NCAP evidence and worker digest rejection.

- Verify native Hardware RPC with the selected Contracts schema, checked-in reference replies and C++ worker.
- Compile the stale-generation RPC test worker before pytest on Windows CI.

## Tiếng Việt

- Đặt phần chuyển đổi trường RPC nội bộ cùng trình xử lý và ghi rõ trách nhiệm của các mô-đun dịch vụ, trạm và Simulator.
- Đồng bộ tham chiếu với tên kho CIRNET chính thức.
- Chuyển dịch vụ Rust native, định nghĩa Preview nội bộ và các bài kiểm thử sang bố cục Cargo chuẩn.
- Dựng cùng Contracts Next và xác minh mã băm worker Simulator Next tại ranh giới thực thi.
- Giữ Hardware RPC 2.0 cục bộ, vòng đời trạm và hành vi Stop có bảo vệ.
- Tham chiếu CIRNET Lab-First cho ranh giới an toàn trạm.
- Chạy CI thành phần Windows khi push lên master với phụ thuộc riêng tư đã ghim.
- Nêu quyền cần thiết trên từng repo private mà Hardware CI đọc.
- Kiểm chứng cách ly kế hoạch và dữ liệu thu, từ chối dữ liệu NCAP không hợp lệ và mã băm worker sai.
- Kiểm chứng RPC Hardware native bằng schema Contracts đã chọn, phản hồi tham chiếu đã lưu trong kho và worker C++.
- Biên dịch worker kiểm thử generation cũ trước khi chạy pytest trên CI Windows.
