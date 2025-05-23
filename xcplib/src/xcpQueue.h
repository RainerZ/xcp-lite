#pragma once
#define __XCP_QUEUE_h__

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */

#include <stdbool.h>
#include <stdint.h>

// @@@@ t.b.d.
// 1. int64_t size
// 2. flush
//

// Handle for queue
typedef struct tQueueHandleType *tQueueHandle;
#define UNDEFINED_QUEUE_HANDLE NULL

// Buffer acquired from the queue with `QueueAcquire` (producer) or obtained with `QueuePop`/`QueuePeek` (consumer)
typedef struct {
    uint8_t *buffer;
    // void *msg;
    uint16_t size;
} tQueueBuffer;

// Create new heap allocated queue. Free using `QueueDeinit`
extern tQueueHandle QueueInit(int64_t buffer_size);

// Creates a queue inside the user provided buffer.
// This can be used to place the queue inside shared memory to be used by multiple applications
extern tQueueHandle QueueInitFromMemory(void *queue_buffer, int64_t queue_buffer_size, bool clear_queue, int64_t *out_buffer_size);

// Deinitialize queue. Does **not** free user allocated memory provided by `QueueInitFromMemory`
extern void QueueDeinit(tQueueHandle queueHandle);

// Acquire a queue buffer of size bytes
extern tQueueBuffer QueueAcquire(tQueueHandle queueHandle, uint64_t size);

// Push an aquired buffer to the queue
extern void QueuePush(tQueueHandle queueHandle, tQueueBuffer *const handle, bool flush);

// Single consumer: Get next buffer from the queue
/// Buffers must be released in the order they were acquired !!!
extern tQueueBuffer QueuePeek(tQueueHandle queueHandle);

// Multi consumer: Not implemented
// extern tQueueBuffer QueuePop(tQueueHandle handle);

// Release buffer from `QueuePeek` or `QueuePop`
// This is required to notify the queue that it can reuse a memory region.
extern void QueueRelease(tQueueHandle queueHandle, tQueueBuffer *const queueBuffer);

// Get amount of bytes in the queue, 0 if empty
extern uint32_t QueueLevel(tQueueHandle queueHandle);

// Clear queue content
extern void QueueClear(tQueueHandle queueHandle);

// Flush queue content
extern void QueueFlush(tQueueHandle queueHandle);
