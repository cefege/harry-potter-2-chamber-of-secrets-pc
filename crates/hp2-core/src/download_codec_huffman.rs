//! The original download Huffman policy, including its unsorted initial leaves.
//!
//! Native callbacks own the archives and the genuinely constructed bit streams;
//! only the tree and symbol paths belong to this invocation.
use crate::bits;
use crate::download_codec::{CodecIo, CodecResult, Hp2CodecFailure};
use hp2_abi::engine::generated::{FBitReader, FBitWriter};

#[derive(Clone, Copy)]
struct Node {
    ch: i32,
    children: [usize; 2],
}

impl Node {
    fn leaf(ch: i32) -> Self {
        Self {
            ch,
            children: [0; 2],
        }
    }
}

#[derive(Clone, Copy)]
struct Code {
    bits: [u8; 32],
    length: usize,
}

unsafe fn write_table(nodes: &[Node], index: usize, writer: *mut FBitWriter) {
    // SAFETY: the native scope owns a live writer, and these indices describe
    // the bounded tree built from the original 256 leaves.
    unsafe {
        let node = nodes[index];
        bits::writer_write_bit(writer, u8::from(node.ch == -1));
        if node.ch == -1 {
            write_table(nodes, node.children[0], writer);
            write_table(nodes, node.children[1], writer);
        } else {
            let ch = node.ch as u8;
            bits::writer_serialize(writer, &ch, 1);
        }
    }
}

unsafe fn read_table(reader: *mut FBitReader) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(511);
    nodes.push(Node::leaf(-1));
    let mut pending = Vec::with_capacity(256);
    pending.push(0);
    // The decoder accepts the original arbitrary binary table, not just tables
    // with at most 256 distinct leaves. Iteration avoids recursion on input.
    // SAFETY: reader is the actual native-constructed reader for this invocation.
    unsafe {
        while let Some(index) = pending.pop() {
            if bits::reader_read_bit(reader) != 0 {
                let first = nodes.len();
                nodes[index].children = [first, first + 1];
                nodes.push(Node::leaf(-1));
                nodes.push(Node::leaf(-1));
                pending.push(first + 1);
                pending.push(first);
            } else {
                // The original Arctor leaves this byte uninitialized if the
                // read fails. Such uninitialized-read UB is not reproduced.
                let mut ch = 0u8;
                bits::reader_serialize(reader, &mut ch, 1);
                nodes[index].ch = i32::from(ch);
            }
        }
    }
    nodes
}

/// Encode with the original three input passes and exact tree/bit ordering.
///
/// # Safety
/// `io` must retain its live native archives and typed bit-writer scope for this
/// invocation. Archive callbacks must honor their supplied buffer lengths.
pub unsafe fn encode(io: &CodecIo<'_>) -> CodecResult<i32> {
    // SAFETY: the caller supplies the native archive/scope contract. Buffers
    // passed to callbacks are live locals, never references to native owners.
    unsafe {
        let saved_pos = io.tell()?;
        let mut total = 0i32;
        let mut nodes = Vec::with_capacity(511);
        let mut counts = [0i32; 511];
        let mut parents = [0usize; 511];
        let mut huff = [0usize; 256];
        for (ch, entry) in huff.iter_mut().enumerate() {
            nodes.push(Node::leaf(ch as i32));
            *entry = ch;
        }
        while !io.at_end()? {
            let mut ch = 0u8;
            io.read(&mut ch, 1)?;
            counts[ch as usize] = counts[ch as usize].wrapping_add(1);
            total = total.wrapping_add(1);
        }
        io.seek(saved_pos)?;
        // Serialize the actual local: output archives may mutate Total.
        io.write((&raw mut total).cast(), 4)?;

        // CompareHuffman is unused in the original. Only trailing zero-count
        // leaves disappear; zero-count leaves below the last used byte stay.
        let mut active = 256;
        while active > 1 && counts[huff[active - 1]] == 0 {
            active -= 1;
        }
        let leaf_count = active;
        let mut bit_count = (active as i32) * 9;
        while active > 1 {
            let first = huff[active - 1];
            let second = huff[active - 2];
            active -= 2;
            let index = nodes.len();
            nodes.push(Node {
                ch: -1,
                children: [first, second],
            });
            parents[first] = index;
            parents[second] = index;
            counts[index] = counts[first].wrapping_add(counts[second]);
            let mut insertion = 0;
            while insertion < active && counts[huff[insertion]] >= counts[index] {
                insertion += 1;
            }
            huff.copy_within(insertion..active, insertion + 1);
            huff[insertion] = index;
            active += 1;
            bit_count = bit_count.wrapping_add(1);
        }
        let root = huff[0];

        // Parent edges give exactly PrependBit's root-first paths without an
        // allocation per node or repeated insertion into descendant vectors.
        let mut codes = [Code {
            bits: [0; 32],
            length: 0,
        }; 256];
        let mut reverse_path = [0u8; 255];
        for (ch, code) in codes[..leaf_count].iter_mut().enumerate() {
            let mut index = ch;
            while index != root {
                let parent = parents[index];
                reverse_path[code.length] = u8::from(nodes[parent].children[1] == index);
                code.length += 1;
                index = parent;
            }
            for position in 0..code.length {
                code.bits[position / 8] |=
                    reverse_path[code.length - position - 1] << (position % 8);
            }
        }

        while !io.at_end()? {
            let mut ch = 0u8;
            io.read(&mut ch, 1)?;
            bit_count = bit_count.wrapping_add(codes[ch as usize].length as i32);
        }
        io.seek(saved_pos)?;

        let writer = io.bit_writer_begin(bit_count)?;
        write_table(&nodes, root, writer);
        while !io.at_end()? {
            let mut ch = 0u8;
            io.read(&mut ch, 1)?;
            let code = &codes[ch as usize];
            for position in 0..code.length {
                bits::writer_write_bit(writer, (code.bits[position / 8] >> (position % 8)) & 1);
            }
        }
        io.check(
            Hp2CodecFailure::HuffmanWriterError,
            (*writer).ar_is_error == 0,
        )?;
        io.check(
            Hp2CodecFailure::HuffmanWriterBits,
            bits::writer_get_num_bits(writer) == bit_count,
        )?;
        io.write(
            bits::writer_get_data(writer),
            bits::writer_get_num_bytes(writer),
        )?;
        Ok(1)
    }
}

/// Decode with the original per-symbol reader check and notification boundary.
///
/// # Safety
/// `io` must retain its live native archives and typed bit-reader scope for this
/// invocation. Archive callbacks must honor their supplied buffer lengths.
pub unsafe fn decode(io: &CodecIo<'_>) -> CodecResult<i32> {
    // SAFETY: as encode. The reader constructor copies the input into its own
    // native buffer; no exclusive native-owner borrow crosses an archive call.
    unsafe {
        let mut total = 0i32;
        io.read((&raw mut total).cast(), 4)?;
        let mut remaining = total;
        let input_length = io.total_size()?.wrapping_sub(io.tell()?);
        io.check_value(
            Hp2CodecFailure::ArrayLength,
            input_length >= 0,
            input_length,
        )?;
        // The native Serialize must initialize this scratch before the reader
        // constructor copies it, just as it must initialize the original TArray.
        let mut input = Vec::<std::mem::MaybeUninit<u8>>::with_capacity(input_length as usize);
        // TRefArray::operator()(0) permits the zero-length boundary. Preserve
        // its null storage pointer rather than Vec's empty dangling pointer.
        let data = if input_length == 0 {
            std::ptr::null_mut()
        } else {
            input.as_mut_ptr().cast::<u8>()
        };
        io.read(data, input_length)?;
        let reader = io.bit_reader_begin(data, input_length.wrapping_mul(8))?;
        let nodes = read_table(reader);
        loop {
            let before_decrement = remaining;
            remaining = remaining.wrapping_sub(1);
            if before_decrement <= 0 {
                break;
            }
            io.check(
                Hp2CodecFailure::HuffmanReaderEnd,
                bits::reader_at_end(reader) == 0,
            )?;
            let mut index = 0;
            while nodes[index].ch == -1 {
                index = nodes[index].children[bits::reader_read_bit(reader) as usize];
            }
            let mut ch = nodes[index].ch as u8;
            io.write(&mut ch, 1)?;
            if io.has_notify()
                && !io.progress(total.wrapping_sub(remaining) as f32 / total as f32)?
            {
                return Ok(0);
            }
        }
        Ok(1)
    }
}
