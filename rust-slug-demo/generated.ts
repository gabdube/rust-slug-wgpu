/**
This code was generated using a LLM. Here's the prompt:

Generate a typescript wrapper for the following rust source:

* Use upper camel case for `MessageType` enum
* Use snake case for the fields of the message data
* Each message type should be a thin wrapper over a DataView using "get" to fetch the field data
* The message struct must have a "get" function for each message data type
* Generate a function `read_output_messages(memory: ArrayBuffer, index_ptr: number): Message[]` that reads messages from the index struct at `index_ptr` in `memory`
* Generate a function `read_vertices(memory: ArrayBuffer, index_ptr: number, data_ptr: number, data_size: number): Uint8Array` that return a view over the vertex data in the index struct
* Generate a function `read_indices(memory: ArrayBuffer, index_ptr: number, data_ptr: number, data_size: number): Uint8Array` that return a view over the index data  in the index struct
* Generate a function `read_font_data(memory: ArrayBuffer, index_ptr: number, data_ptr: number, data_size: number): Uint8Array` that return a view over the font data  in the index struct

```rust
[source code of shared.rs]
```
*/
export enum MessageType {
    UpdateVertexBuffer = 0,
    UpdateFontAtlas = 1,
    DrawText = 2,
    InvalidateText = 3,
}

export class DrawTextParams {
    constructor(private view: DataView, private offset: number) {}

    get indices_count(): number {
        return this.view.getUint32(this.offset + 0, true);
    }

    get first_index(): number {
        return this.view.getUint32(this.offset + 4, true);
    }

    get font_id(): number {
        return this.view.getUint32(this.offset + 8, true);
    }
}

export class UpdateVertexParams {
    constructor(private view: DataView, private offset: number) {}

    get data_size(): number {
        return this.view.getUint32(this.offset + 0, true);
    }

    get indices_size(): number {
        return this.view.getUint32(this.offset + 4, true);
    }

    get vertices_size(): number {
        return this.view.getUint32(this.offset + 8, true);
    }
}

export class UpdateFontAtlasParams {
    constructor(private view: DataView, private offset: number) {}

    get font_id(): number {
        return this.view.getUint32(this.offset + 0, true);
    }

    get data_offset(): number {
        return this.view.getUint32(this.offset + 4, true);
    }

    get data_size(): number {
        return this.view.getUint32(this.offset + 8, true);
    }

    get curves_offset(): number {
        return this.view.getUint32(this.offset + 12, true);
    }

    get curves_size(): number {
        return this.view.getUint32(this.offset + 16, true);
    }

    get curves_indices_offset(): number {
        return this.view.getUint32(this.offset + 20, true);
    }

    get curves_indices_size(): number {
        return this.view.getUint32(this.offset + 24, true);
    }

    get glyphs_offset(): number {
        return this.view.getUint32(this.offset + 28, true);
    }

    get glyphs_size(): number {
        return this.view.getUint32(this.offset + 32, true);
    }
}

export class Message {
    constructor(private view: DataView, private offset: number) {}

    get ty(): MessageType {
        return this.view.getUint32(this.offset + 0, true) as MessageType;
    }

    get draw_text(): DrawTextParams {
        return new DrawTextParams(this.view, this.offset + 4);
    }

    get update_vertex(): UpdateVertexParams {
        return new UpdateVertexParams(this.view, this.offset + 4);
    }

    get update_font_atlas(): UpdateFontAtlasParams {
        return new UpdateFontAtlasParams(this.view, this.offset + 4);
    }
}

/**
 * Reads messages from the index struct at `index_ptr` in memory
 */
export function read_output_messages(memory: ArrayBuffer, index_ptr: number): Message[] {
    const view = new DataView(memory);
    const message_count = view.getUint32(index_ptr + 0, true);
    const messages_ptr = view.getUint32(index_ptr + 4, true);

    const messages: Message[] = [];
    // The size of a Message is 40 bytes (4 bytes for the type enum + 36 bytes for the largest union variant)
    const MESSAGE_SIZE = 40; 

    for (let i = 0; i < message_count; i++) {
        messages.push(new Message(view, messages_ptr + i * MESSAGE_SIZE));
    }

    return messages;
}

/**
 * Returns a view over the vertex data pointed to by the index struct
 */
export function read_vertices(memory: ArrayBuffer, index_ptr: number, data_ptr: number, data_size: number): Uint8Array {
    const view = new DataView(memory);
    // vertices_ptr is located at offset 8 in OutputIndex
    const vertices_ptr = view.getUint32(index_ptr + 8, true);
    return new Uint8Array(memory, vertices_ptr + data_ptr, data_size);
}

/**
 * Returns a view over the index data pointed to by the index struct
 */
export function read_indices(memory: ArrayBuffer, index_ptr: number, data_ptr: number, data_size: number): Uint8Array {
    const view = new DataView(memory);
    // indices_ptr is located at offset 12 in OutputIndex
    const indices_ptr = view.getUint32(index_ptr + 12, true);
    return new Uint8Array(memory, indices_ptr + data_ptr, data_size);
}

/**
 * Returns a view over the font data pointed to by the index struct
 */
export function read_font_data(memory: ArrayBuffer, index_ptr: number, data_ptr: number, data_size: number): Uint8Array {
    const view = new DataView(memory);
    // font_data_ptr is located at offset 16 in OutputIndex
    const font_data_ptr = view.getUint32(index_ptr + 16, true);
    return new Uint8Array(memory, font_data_ptr + data_ptr, data_size);
}
