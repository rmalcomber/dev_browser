import { signal } from '@preact/signals';

/** The id of the workspace the shell is showing. Set from Rust over IPC (M1.3); the shell never invents it. */
export const activeWorkspace = signal<string | null>(null);
