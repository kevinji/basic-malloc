use std::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
    mem, ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

#[derive(Debug)]
struct BlockMeta {
    size: usize,
    is_free: bool,
    next: *mut BlockMeta,
}

static BASE: AtomicPtr<BlockMeta> = AtomicPtr::new(ptr::null_mut());

impl BlockMeta {
    fn find_free_block(size: usize) -> (*mut Self, *mut Self) {
        let mut current = BASE.load(Ordering::Acquire);
        let mut last = current;
        while !current.is_null() {
            last = current;
            unsafe {
                if (*current).is_free && (*current).size >= size {
                    return (current, last);
                }
                current = (*current).next;
            }
        }
        (current, last)
    }

    fn request_space(size: usize) -> *mut Self {
        let total_size = size + mem::size_of::<BlockMeta>();
        let block_ptr = unsafe { libc::sbrk(total_size.try_into().unwrap()) };
        if block_ptr == (usize::MAX as *mut c_void) {
            // sbrk failed
            return ptr::null_mut();
        }
        let meta_ptr = block_ptr as *mut BlockMeta;
        unsafe {
            (*meta_ptr).size = size;
            (*meta_ptr).is_free = false;
            (*meta_ptr).next = ptr::null_mut();
        }
        meta_ptr
    }

    fn claim_block(size: usize) -> *mut Self {
        if BASE.load(Ordering::Acquire).is_null() {
            let block = BlockMeta::request_space(size);
            if block.is_null() {
                return ptr::null_mut();
            }

            BASE.store(block, Ordering::Release);
            return block;
        }

        let (block, last) = BlockMeta::find_free_block(size);
        if block.is_null() {
            let block = BlockMeta::request_space(size);
            if block.is_null() {
                return ptr::null_mut();
            }

            unsafe {
                (*last).next = block;
            }
            return block;
        }

        unsafe {
            (*block).is_free = false;
        }
        block
    }

    fn get_data_ptr(ptr: *mut BlockMeta) -> *mut u8 {
        (unsafe { ptr.add(1) } as *mut u8)
    }

    fn get_meta_ptr(ptr: *mut u8) -> *mut BlockMeta {
        unsafe { (ptr as *mut BlockMeta).sub(1) }
    }
}

fn alloc_impl(size: usize) -> *mut u8 {
    let block = BlockMeta::claim_block(size);
    if block.is_null() {
        return ptr::null_mut();
    }
    BlockMeta::get_data_ptr(block)
}

fn dealloc_impl(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }

    let meta_ptr = BlockMeta::get_meta_ptr(ptr);
    unsafe {
        (*meta_ptr).is_free = true;
    }
}

pub struct BasicAllocator;

unsafe impl GlobalAlloc for BasicAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        alloc_impl(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        dealloc_impl(ptr);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn malloc(size: usize) -> *mut c_void {
    let layout = Layout::from_size_align(size, mem::align_of::<usize>()).unwrap();
    (unsafe { BasicAllocator.alloc(layout) } as *mut c_void)
}

#[unsafe(no_mangle)]
pub extern "C" fn free(ptr: *mut c_void) {
    dealloc_impl(ptr as *mut u8);
}

#[unsafe(no_mangle)]
pub extern "C" fn calloc(num: usize, size: usize) -> *mut c_void {
    let layout = Layout::from_size_align(num * size, mem::align_of::<usize>()).unwrap();
    (unsafe { BasicAllocator.alloc_zeroed(layout) } as *mut c_void)
}

#[unsafe(no_mangle)]
pub extern "C" fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if ptr.is_null() {
        return malloc(size);
    }

    let meta_ptr = BlockMeta::get_meta_ptr(ptr as *mut u8);
    let current_size = unsafe { (*meta_ptr).size };

    let layout = Layout::from_size_align(current_size, mem::align_of::<usize>()).unwrap();
    (unsafe { BasicAllocator.realloc(ptr as *mut u8, layout, size) } as *mut c_void)
}
