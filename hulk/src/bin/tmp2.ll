; ModuleID = 'tmp2'
source_filename = "tmp2"

@print_fmt_num = private unnamed_addr constant [4 x i8] c"%f\0A\00", align 1

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
  %vec_data_slot1 = getelementptr inbounds { i64, ptr }, ptr %vec_alloc, i32 0, i32 1
  %data_ptr = load ptr, ptr %vec_data_slot1, align 8
  %elem_i8 = getelementptr inbounds i8, ptr %data_ptr, i64 ptrtoint (ptr getelementptr (double, ptr null, i32 1) to i64)
  %load_elem = load double, ptr %elem_i8, align 8
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_num, double %load_elem)
  ret double %load_elem
}

declare ptr @malloc(i64)

declare i32 @printf(ptr, ...)
