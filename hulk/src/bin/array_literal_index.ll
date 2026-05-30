; ModuleID = 'array_literal_index'
source_filename = "array_literal_index"

@index_panic_msg_0 = private unnamed_addr constant [35 x i8] c"Runtime error: index out of bounds\00", align 1
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
  %a = alloca ptr, align 8
  store ptr %vec_alloc, ptr %a, align 8
  %load_a = load ptr, ptr %a, align 8
  %vec_data_slot1 = getelementptr inbounds { i64, ptr }, ptr %load_a, i32 0, i32 1
  %data_ptr = load ptr, ptr %vec_data_slot1, align 8
  %vec_len_slot2 = getelementptr inbounds { i64, ptr }, ptr %load_a, i32 0, i32 0
  %vec_len = load i64, ptr %vec_len_slot2, align 4
  %idx_lt_len = icmp ult i64 1, %vec_len
  %idx_valid = and i1 true, %idx_lt_len
  br i1 %idx_valid, label %index_ok, label %index_fail

index_ok:                                         ; preds = %entry
  %elem_i8 = getelementptr inbounds i8, ptr %data_ptr, i64 ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64)
  %load_elem = load double, ptr %elem_i8, align 8
  br label %index_cont

index_fail:                                       ; preds = %entry
  call void @hulk_panic(ptr @index_panic_msg_0)
  unreachable

index_cont:                                       ; preds = %index_ok
  %format_num = call ptr @hulk_format_number(double %load_elem)
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr %format_num)
  ret double %load_elem
}

declare ptr @malloc(i64)

declare void @hulk_panic(ptr)

declare i32 @printf(ptr, ...)

declare ptr @hulk_format_number(double)
