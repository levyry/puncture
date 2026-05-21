import zlib
import argparse
import os

def compress_file(input_filename, use_fixed_huffman):
    output_filename = f"{input_filename}.gz"
    
    strategy = zlib.Z_FIXED if use_fixed_huffman else zlib.Z_DEFAULT_STRATEGY

    compressor = zlib.compressobj(
        level=9,
        method=zlib.DEFLATED,
        wbits=31,
        memLevel=8,
        strategy=strategy
    )
    
    try:
        with open(input_filename, "rb") as f_in, open(output_filename, "wb") as f_out:
            while chunk := f_in.read(8192):
                compressed_data = compressor.compress(chunk)
                if compressed_data:
                    f_out.write(compressed_data)
            
            f_out.write(compressor.flush())
            
        
    except FileNotFoundError:
        print(f"Error: The input file '{input_filename}' was not found.")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Compress a file using zlib (gzip format).")
    
    parser.add_argument("input_file", help="Path to the file you want to compress.")
    
    parser.add_argument(
        "--fixed", 
        action="store_true", 
        help="Force Fixed Huffman encoding. If omitted, uses Dynamic Huffman."
    )
    
    args = parser.parse_args()
    
    compress_file(args.input_file, args.fixed)