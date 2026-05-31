; ModuleID = 'vector_of_objects'
source_filename = "vector_of_objects"

%vtable.Person = type { i64, ptr }
%obj.Person = type { ptr, ptr }

@vtable.Person = constant %vtable.Person { i64 0, ptr @Person.toString }
@str = private unnamed_addr constant [6 x i8] c"Alice\00", align 1
@str.1 = private unnamed_addr constant [4 x i8] c"Bob\00", align 1
@vec_open_0 = private unnamed_addr constant [2 x i8] c"[\00", align 1
@vec_close_1 = private unnamed_addr constant [2 x i8] c"]\00", align 1
@vec_sep_2 = private unnamed_addr constant [3 x i8] c", \00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define ptr @Person.toString(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %load_self = load ptr, ptr %self1, align 8
  %name_field = getelementptr inbounds %obj.Person, ptr %load_self, i32 0, i32 1
  %load_name = load ptr, ptr %name_field, align 8
  ret ptr %load_name
}

define double @main() {
entry:
  %vec_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr ({ i64, ptr }, ptr null, i32 1) to i64))
  %data_alloc = call ptr @malloc(i64 mul (i64 ptrtoint (ptr getelementptr (ptr, ptr null, i32 1) to i64), i64 2))
  %vec_len_slot = getelementptr inbounds { i64, ptr }, ptr %vec_alloc, i32 0, i32 0
  store i64 2, ptr %vec_len_slot, align 4
  %vec_data_slot = getelementptr inbounds { i64, ptr }, ptr %vec_alloc, i32 0, i32 1
  store ptr %data_alloc, ptr %vec_data_slot, align 8
  %obj_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Person, ptr null, i32 1) to i64))
  %vtable_slot = getelementptr inbounds %obj.Person, ptr %obj_alloc, i32 0, i32 0
  store ptr @vtable.Person, ptr %vtable_slot, align 8
  %self = alloca ptr, align 8
  store ptr %obj_alloc, ptr %self, align 8
  %name = alloca ptr, align 8
  store ptr @str, ptr %name, align 8
  %load_name = load ptr, ptr %name, align 8
  %name_field = getelementptr inbounds %obj.Person, ptr %obj_alloc, i32 0, i32 1
  store ptr %load_name, ptr %name_field, align 8
  %elem_i8_0 = getelementptr inbounds i8, ptr %data_alloc, i64 0
  store ptr %obj_alloc, ptr %elem_i8_0, align 8
  %obj_alloc1 = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Person, ptr null, i32 1) to i64))
  %vtable_slot2 = getelementptr inbounds %obj.Person, ptr %obj_alloc1, i32 0, i32 0
  store ptr @vtable.Person, ptr %vtable_slot2, align 8
  %self3 = alloca ptr, align 8
  store ptr %obj_alloc1, ptr %self3, align 8
  %name4 = alloca ptr, align 8
  store ptr @str.1, ptr %name4, align 8
  %load_name5 = load ptr, ptr %name4, align 8
  %name_field6 = getelementptr inbounds %obj.Person, ptr %obj_alloc1, i32 0, i32 1
  store ptr %load_name5, ptr %name_field6, align 8
  %elem_i8_1 = getelementptr inbounds i8, ptr %data_alloc, i64 ptrtoint (ptr getelementptr (ptr, ptr null, i32 1) to i64)
  store ptr %obj_alloc1, ptr %elem_i8_1, align 8
  %people = alloca ptr, align 8
  store ptr %vec_alloc, ptr %people, align 8
  %load_people = load ptr, ptr %people, align 8
  %vec_str_len_slot = getelementptr inbounds { i64, ptr }, ptr %load_people, i32 0, i32 0
  %vec_str_len = load i64, ptr %vec_str_len_slot, align 4
  %vec_str_data_slot = getelementptr inbounds { i64, ptr }, ptr %load_people, i32 0, i32 1
  %vec_str_data = load ptr, ptr %vec_str_data_slot, align 8
  %vec_render = alloca ptr, align 8
  store ptr @vec_open_0, ptr %vec_render, align 8
  %vec_render_idx = alloca i64, align 8
  store i64 0, ptr %vec_render_idx, align 4
  br label %vec_render_cond

vec_render_cond:                                  ; preds = %vec_render_append, %entry
  %vec_render_idx_val = load i64, ptr %vec_render_idx, align 4
  %vec_render_cmp = icmp ult i64 %vec_render_idx_val, %vec_str_len
  br i1 %vec_render_cmp, label %vec_render_body, label %vec_render_after

vec_render_body:                                  ; preds = %vec_render_cond
  %vec_render_byte_offset = mul i64 %vec_render_idx_val, ptrtoint (ptr getelementptr (ptr, ptr null, i32 1) to i64)
  %vec_render_elem_i8 = getelementptr inbounds i8, ptr %vec_str_data, i64 %vec_render_byte_offset
  %vec_render_elem_obj = load ptr, ptr %vec_render_elem_i8, align 8
  %call_toString = call ptr @Person.toString(ptr %vec_render_elem_obj)
  %vec_render_need_sep = icmp ne i64 %vec_render_idx_val, 0
  br i1 %vec_render_need_sep, label %vec_render_sep, label %vec_render_append

vec_render_after:                                 ; preds = %vec_render_cond
  %vec_render_done = load ptr, ptr %vec_render, align 8
  %concat8 = call ptr @hulk_concat(ptr %vec_render_done, ptr @vec_close_1)
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr %concat8)
  ret double 0.000000e+00

vec_render_sep:                                   ; preds = %vec_render_body
  %vec_render_current = load ptr, ptr %vec_render, align 8
  %concat = call ptr @hulk_concat(ptr %vec_render_current, ptr @vec_sep_2)
  store ptr %concat, ptr %vec_render, align 8
  br label %vec_render_append

vec_render_append:                                ; preds = %vec_render_sep, %vec_render_body
  %vec_render_current2 = load ptr, ptr %vec_render, align 8
  %concat7 = call ptr @hulk_concat(ptr %vec_render_current2, ptr %call_toString)
  store ptr %concat7, ptr %vec_render, align 8
  %vec_render_next = add i64 %vec_render_idx_val, 1
  store i64 %vec_render_next, ptr %vec_render_idx, align 4
  br label %vec_render_cond
}

declare ptr @malloc(i64)

declare i32 @printf(ptr, ...)

declare ptr @hulk_concat(ptr, ptr)
