; ModuleID = 'tmp_comp'
source_filename = "tmp_comp"

@print_fmt_num = private unnamed_addr constant [4 x i8] c"%f\0A\00", align 1

define double @main() {
entry:
  %vec_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr ({ i64, ptr }, ptr null, i32 1) to i64))
  %data_alloc = call ptr @malloc(i64 mul (i64 ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64), i64 4))
  %vec_len_slot = getelementptr inbounds { i64, ptr }, ptr %vec_alloc, i32 0, i32 0
  store i64 4, ptr %vec_len_slot, align 4
  %vec_data_slot = getelementptr inbounds { i64, ptr }, ptr %vec_alloc, i32 0, i32 1
  store ptr %data_alloc, ptr %vec_data_slot, align 8
  %elem_i8_0 = getelementptr inbounds i8, ptr %data_alloc, i64 0
  store double 1.000000e+00, ptr %elem_i8_0, align 8
  %elem_i8_1 = getelementptr inbounds i8, ptr %data_alloc, i64 ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64)
  store double 2.000000e+00, ptr %elem_i8_1, align 8
  %elem_i8_2 = getelementptr inbounds i8, ptr %data_alloc, i64 mul (i64 ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64), i64 2)
  store double 3.000000e+00, ptr %elem_i8_2, align 8
  %elem_i8_3 = getelementptr inbounds i8, ptr %data_alloc, i64 mul (i64 ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64), i64 3)
  store double 4.000000e+00, ptr %elem_i8_3, align 8
  %nums = alloca ptr, align 8
  store ptr %vec_alloc, ptr %nums, align 8
  %load_nums = load ptr, ptr %nums, align 8
  %src_len_slot = getelementptr inbounds { i64, ptr }, ptr %load_nums, i32 0, i32 0
  %src_len = load i64, ptr %src_len_slot, align 4
  %vec_alloc1 = call ptr @malloc(i64 ptrtoint (ptr getelementptr ({ i64, ptr }, ptr null, i32 1) to i64))
  %data_size = mul i64 ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64), %src_len
  %data_alloc2 = call ptr @malloc(i64 %data_size)
  %dst_len_slot = getelementptr inbounds { i64, ptr }, ptr %vec_alloc1, i32 0, i32 0
  store i64 %src_len, ptr %dst_len_slot, align 4
  %dst_data_slot = getelementptr inbounds { i64, ptr }, ptr %vec_alloc1, i32 0, i32 1
  store ptr %data_alloc2, ptr %dst_data_slot, align 8
  %comp_idx = alloca i64, align 8
  store i64 0, ptr %comp_idx, align 4
  br label %comp_cond

comp_cond:                                        ; preds = %comp_body, %entry
  %idx_val = load i64, ptr %comp_idx, align 4
  %cmp = icmp ult i64 %idx_val, %src_len
  br i1 %cmp, label %comp_body, label %comp_after

comp_body:                                        ; preds = %comp_cond
  %byte_offset = mul i64 %idx_val, ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64)
  %src_elem_i8 = getelementptr inbounds i8, ptr %data_alloc2, i64 %byte_offset
  %load_src_elem = load double, ptr %src_elem_i8, align 8
  %x = alloca double, align 8
  store double %load_src_elem, ptr %x, align 8
  %load_x = load double, ptr %x, align 8
  %multmp = fmul double %load_x, 2.000000e+00
  %dst_elem_i8 = getelementptr inbounds i8, ptr %data_alloc2, i64 %byte_offset
  store double %multmp, ptr %dst_elem_i8, align 8
  %idx_next = add i64 %idx_val, 1
  store i64 %idx_next, ptr %comp_idx, align 4
  br label %comp_cond

comp_after:                                       ; preds = %comp_cond
  %doubled = alloca ptr, align 8
  store ptr %vec_alloc1, ptr %doubled, align 8
  %load_doubled = load ptr, ptr %doubled, align 8
  %vec_data_slot3 = getelementptr inbounds { i64, ptr }, ptr %load_doubled, i32 0, i32 1
  %data_ptr = load ptr, ptr %vec_data_slot3, align 8
  %elem_i8 = getelementptr inbounds i8, ptr %data_ptr, i64 mul (i64 ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64), i64 2)
  %load_elem = load double, ptr %elem_i8, align 8
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_num, double %load_elem)
  ret double %load_elem
}

declare ptr @malloc(i64)

declare i32 @printf(ptr, ...)
