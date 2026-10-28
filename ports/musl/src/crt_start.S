.text
.global _start
.type _start,@function
_start:
	mov %rsp, %rdi
	andq $-16, %rsp
	call __wovenhat_start_c
	ud2
