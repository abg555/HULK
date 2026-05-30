; ModuleID = 'comp_test'
source_filename = "comp_test"

@vec_open_0 = private unnamed_addr constant [2 x i8] c"[\00", align 1
@vec_close_1 = private unnamed_addr constant [2 x i8] c"]\00", align 1
@vec_sep_2 = private unnamed_addr constant [3 x i8] c", \00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define double @main() {
entry:
  %vec_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr ({ i64, ptr }, ptr null, i32 1) to i64))
  %data_alloc = call ptr @malloc(i64 mul (i64 ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64), i64 3))
  %vec_len_slot = getelementptr inbounds { i64, ptr }, ptr %vec_alloc, i32 0, i32 0
  store i64 3, ptr %vec_len_slot, align 4
  %vec_data_slot = getelementptr inbounds { i64, ptr }, ptr %vec_alloc, i32 0, i32 1
  store ptr %data_alloc, ptr %vec_data_slot, align 8
  %elem_i8_0 = getelementptr inbounds i8, ptr %data_alloc, i64 0
  store double 1.000000e+00, ptr %elem_i8_0, align 8
  %elem_i8_1 = getelementptr inbounds i8, ptr %data_alloc, i64 ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64)
  store double 2.000000e+00, ptr %elem_i8_1, align 8
  %elem_i8_2 = getelementptr inbounds i8, ptr %data_alloc, i64 mul (i64 ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64), i64 2)
  store double 3.000000e+00, ptr %elem_i8_2, align 8
  %src_len_slot = getelementptr inbounds { i64, ptr }, ptr %vec_alloc, i32 0, i32 0
  %src_len = load i64, ptr %src_len_slot, align 4
  %src_data_slot = getelementptr inbounds { i64, ptr }, ptr %vec_alloc, i32 0, i32 1
  %src_data_ptr = load ptr, ptr %src_data_slot, align 8
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
  %src_elem_i8 = getelementptr inbounds i8, ptr %src_data_ptr, i64 %byte_offset
  %load_src_elem = load double, ptr %src_elem_i8, align 8
  %x = alloca double, align 8
  store double %load_src_elem, ptr %x, align 8
  %load_x = load double, ptr %x, align 8
  %dst_elem_i8 = getelementptr inbounds i8, ptr %data_alloc2, i64 %byte_offset
  store double %load_x, ptr %dst_elem_i8, align 8
  %idx_next = add i64 %idx_val, 1
  store i64 %idx_next, ptr %comp_idx, align 4
  br label %comp_cond

comp_after:                                       ; preds = %comp_cond
  %dest = alloca ptr, align 8
  store ptr %vec_alloc1, ptr %dest, align 8
  %load_dest = load ptr, ptr %dest, align 8
  %vec_str_len_slot = getelementptr inbounds { i64, ptr }, ptr %load_dest, i32 0, i32 0
  %vec_str_len = load i64, ptr %vec_str_len_slot, align 4
  %vec_str_data_slot = getelementptr inbounds { i64, ptr }, ptr %load_dest, i32 0, i32 1
  %vec_str_data = load ptr, ptr %vec_str_data_slot, align 8
  %vec_render = alloca ptr, align 8
  store ptr @vec_open_0, ptr %vec_render, align 8
  %vec_render_idx = alloca i64, align 8
  store i64 0, ptr %vec_render_idx, align 4
  br label %vec_render_cond

vec_render_cond:                                  ; preds = %vec_render_append, %comp_after
  %vec_render_idx_val = load i64, ptr %vec_render_idx, align 4
  %vec_render_cmp = icmp ult i64 %vec_render_idx_val, %vec_str_len
  br i1 %vec_render_cmp, label %vec_render_body, label %vec_render_after

vec_render_body:                                  ; preds = %vec_render_cond
  %vec_render_byte_offset = mul i64 %vec_render_idx_val, ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64)
  %vec_render_elem_i8 = getelementptr inbounds i8, ptr %vec_str_data, i64 %vec_render_byte_offset
  %vec_render_elem_num = load double, ptr %vec_render_elem_i8, align 8
  %format_num = call ptr @hulk_format_number(double %vec_render_elem_num)
  %vec_render_need_sep = icmp ne i64 %vec_render_idx_val, 0
  br i1 %vec_render_need_sep, label %vec_render_sep, label %vec_render_append

vec_render_after:                                 ; preds = %vec_render_cond
  %vec_render_done = load ptr, ptr %vec_render, align 8
  %concat4 = call ptr @hulk_concat(ptr %vec_render_done, ptr @vec_close_1)
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr %concat4)
  ret double 0.000000e+00

vec_render_sep:                                   ; preds = %vec_render_body
  %vec_render_current = load ptr, ptr %vec_render, align 8
  %concat = call ptr @hulk_concat(ptr %vec_render_current, ptr @vec_sep_2)
  store ptr %concat, ptr %vec_render, align 8
  br label %vec_render_append

vec_render_append:                                ; preds = %vec_render_sep, %vec_render_body
  %vec_render_current2 = load ptr, ptr %vec_render, align 8
  %concat3 = call ptr @hulk_concat(ptr %vec_render_current2, ptr %format_num)
  store ptr %concat3, ptr %vec_render, align 8
  %vec_render_next = add i64 %vec_render_idx_val, 1
  store i64 %vec_render_next, ptr %vec_render_idx, align 4
  br label %vec_render_cond
}

declare ptr @malloc(i64)

declare i32 @printf(ptr, ...)

declare ptr @hulk_format_number(double)

declare ptr @hulk_concat(ptr, ptr)
