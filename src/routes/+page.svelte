<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { onMount } from "svelte";

	interface FileInfo {
		name: string;
		path: string;
		is_dir: boolean;
		size?: number;
		modified?: number;
	}

	let sourcePath = "X:\\Innhealth\\Gait\\L";
	let destinationPath = "X:\\Innhealth\\Gait\\IH-0777-B";
	let files: FileInfo[] = [];
	let result = "";
	let loading = false;
	let selectedFiles: Set<string> = new Set(); // Track multiple selected files by path

	async function listDirectory() {
		try {
			loading = true;
			result = "";
			files = await invoke("list_directory", { dirPath: sourcePath });
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}

	async function moveSelectedFiles() {
		if (selectedFiles.size === 0) {
			result = "Please select at least one file first";
			return;
		}

		try {
			loading = true;
			let successCount = 0;
			let errorCount = 0;
			const errors: string[] = [];

			for (const filePath of selectedFiles) {
				const file = files.find(f => f.path === filePath);
				if (file && !file.is_dir) {
					try {
						const destPath = `${destinationPath}\\${file.name}`;
						await invoke("move_file", { 
							sourcePath: file.path, 
							destinationPath: destPath
						});
						successCount++;
					} catch (e) {
						errorCount++;
						errors.push(`${file.name}: ${e}`);
					}
				}
			}

			// Clear selection after moving
			selectedFiles.clear();

			if (errorCount === 0) {
				result = `Successfully moved ${successCount} file(s) to ${destinationPath}`;
			} else {
				result = `Moved ${successCount} file(s), ${errorCount} failed:\n${errors.join('\n')}`;
			}

			// Refresh the list after moving
			await listDirectory();
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}

	async function copySelectedFiles() {
		if (selectedFiles.size === 0) {
			result = "Please select at least one file first";
			return;
		}

		try {
			loading = true;
			let successCount = 0;
			let errorCount = 0;
			const errors: string[] = [];

			for (const filePath of selectedFiles) {
				const file = files.find(f => f.path === filePath);
				if (file && !file.is_dir) {
					try {
						const destPath = `${destinationPath}\\${file.name}`;
						await invoke("copy_file", { 
							sourcePath: file.path, 
							destinationPath: destPath
						});
						successCount++;
					} catch (e) {
						errorCount++;
						errors.push(`${file.name}: ${e}`);
					}
				}
			}

			if (errorCount === 0) {
				result = `Successfully copied ${successCount} file(s) to ${destinationPath}`;
			} else {
				result = `Copied ${successCount} file(s), ${errorCount} failed:\n${errors.join('\n')}`;
			}
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}

	async function moveFile() {
		const selectedFile = files.find(f => f.path === Array.from(selectedFiles)[0]);
		if (!selectedFile) {
			result = "Please select a file first";
			return;
		}

		try {
			loading = true;
			const destPath = `${destinationPath}\\${selectedFile.name}`;
			result = await invoke("move_file", { 
				sourcePath: selectedFile.path, 
				destinationPath: destPath
			});
			// Refresh the list after moving
			await listDirectory();
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}

	async function copyFile() {
		const selectedFile = files.find(f => f.path === Array.from(selectedFiles)[0]);
		if (!selectedFile) {
			result = "Please select a file first";
			return;
		}

		try {
			loading = true;
			const destPath = `${destinationPath}\\${selectedFile.name}`;
			result = await invoke("copy_file", { 
				sourcePath: selectedFile.path, 
				destinationPath: destPath
			});
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}

	async function checkFileExists() {
		try {
			loading = true;
			const exists = await invoke("file_exists", { filePath: sourcePath });
			result = `Directory exists: ${exists}`;
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}

	function toggleFileSelection(file: FileInfo) {
		if (selectedFiles.has(file.path)) {
			selectedFiles.delete(file.path);
		} else {
			selectedFiles.add(file.path);
		}
		selectedFiles = selectedFiles; // Trigger reactivity
		result = `Selected ${selectedFiles.size} file(s)`;
	}

	function selectAllFiles() {
		const videoFiles = files.filter(f => !f.is_dir && f.name.toLowerCase().includes('.mov'));
		selectedFiles = new Set(videoFiles.map(f => f.path));
		result = `Selected all ${selectedFiles.size} video files`;
	}

	function clearSelection() {
		selectedFiles.clear();
		selectedFiles = selectedFiles; // Trigger reactivity
		result = "Selection cleared";
	}

	function formatFileSize(bytes?: number): string {
		if (!bytes) return "N/A";
		const sizes = ['B', 'KB', 'MB', 'GB'];
		let i = 0;
		let size = bytes;
		while (size >= 1024 && i < sizes.length - 1) {
			size /= 1024;
			i++;
		}
		return `${size.toFixed(1)} ${sizes[i]}`;
	}

	function formatDate(timestamp?: number): string {
		if (!timestamp) return "N/A";
		return new Date(timestamp * 1000).toLocaleString();
	}

	// Auto-load directory on mount
	onMount(() => {
		listDirectory();
	});
</script>

<main class="container">
	<h1>📁 Directory File Manager</h1>

	<div class="controls">
		<div class="form-group">
			<label for="source">Source Directory:</label>
			<input id="source" bind:value={sourcePath} placeholder="Enter source directory path" />
		</div>

		<div class="form-group">
			<label for="destination">Destination Directory:</label>
			<input id="destination" bind:value={destinationPath} placeholder="Enter destination directory path" />
		</div>

		<div class="buttons">
			<button on:click={checkFileExists} disabled={loading}>
				{loading ? 'Checking...' : 'Check Directory'}
			</button>
			
			<button on:click={listDirectory} disabled={loading}>
				{loading ? 'Loading...' : 'List Directory'}
			</button>

			<button on:click={moveSelectedFiles} disabled={loading || selectedFiles.size === 0}>
				{loading ? 'Moving...' : 'Move Selected Files'}
			</button>

			<button on:click={copySelectedFiles} disabled={loading || selectedFiles.size === 0}>
				{loading ? 'Copying...' : 'Copy Selected Files'}
			</button>

			<button on:click={selectAllFiles} disabled={loading || files.length === 0}>
				Select All Videos
			</button>

			<button on:click={clearSelection} disabled={loading || selectedFiles.size === 0}>
				Clear Selection
			</button>
		</div>
	</div>

	{#if result}
		<div class="result">
			<pre>{result}</pre>
		</div>
	{/if}

	{#if files.length > 0}
		<div class="file-list">
			<h3>📂 Contents of: {sourcePath}</h3>
			<div class="selection-info">
				Selected: {selectedFiles.size} file(s)
			</div>
			<div class="file-grid">
				{#each files as file}
					<div class="file-item" class:selected={selectedFiles.has(file.path)} on:click={() => toggleFileSelection(file)}>
						<div class="file-checkbox">
							<input type="checkbox" checked={selectedFiles.has(file.path)} on:change={() => toggleFileSelection(file)} />
						</div>
						<div class="file-icon">
							{file.is_dir ? '📁' : '📄'}
						</div>
						<div class="file-info">
							<div class="file-name">{file.name}</div>
							<div class="file-details">
								{#if !file.is_dir && file.size}
									<span class="file-size">{formatFileSize(file.size)}</span>
								{/if}
								{#if file.modified}
									<span class="file-date">{formatDate(file.modified)}</span>
								{/if}
							</div>
						</div>
					</div>
				{/each}
			</div>
		</div>
	{:else if !loading}
		<div class="empty-state">
			<p>No files found in directory. Click "List Directory" to load contents.</p>
		</div>
	{/if}
</main>

<style>
	.container {
		max-width: 1200px;
		margin: 0 auto;
		padding: 2rem;
	}

	h1 {
		text-align: center;
		color: #333;
		margin-bottom: 2rem;
	}

	.controls {
		margin-bottom: 2rem;
		padding: 1rem;
		background: #f8f9fa;
		border-radius: 8px;
	}

	.form-group {
		margin-bottom: 1rem;
	}

	label {
		display: block;
		margin-bottom: 0.5rem;
		font-weight: bold;
		color: #555;
	}

	input {
		width: 100%;
		padding: 0.75rem;
		border: 1px solid #ddd;
		border-radius: 4px;
		font-family: monospace;
		font-size: 0.9rem;
	}

	.buttons {
		margin-top: 1rem;
		display: flex;
		gap: 1rem;
		flex-wrap: wrap;
	}

	button {
		padding: 0.75rem 1.5rem;
		background: #007acc;
		color: white;
		border: none;
		border-radius: 4px;
		cursor: pointer;
		font-weight: 500;
		transition: background-color 0.2s;
	}

	button:hover:not(:disabled) {
		background: #005a9e;
	}

	button:disabled {
		background: #ccc;
		cursor: not-allowed;
	}

	.result {
		margin: 1rem 0;
		padding: 1rem;
		background: #f5f5f5;
		border-radius: 4px;
		border-left: 4px solid #007acc;
	}

	pre {
		margin: 0;
		white-space: pre-wrap;
		word-break: break-all;
	}

	.file-list {
		margin-top: 2rem;
	}

	.file-list h3 {
		margin-bottom: 1rem;
		color: #333;
	}

	.selection-info {
		margin-bottom: 1rem;
		padding: 0.5rem 1rem;
		background: #e3f2fd;
		border-radius: 4px;
		color: #007acc;
		font-weight: 500;
	}

	.file-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
		gap: 1rem;
		margin-bottom: 2rem;
	}

	.file-item {
		display: flex;
		align-items: center;
		padding: 1rem;
		border: 2px solid #e9ecef;
		border-radius: 8px;
		cursor: pointer;
		transition: all 0.2s;
		background: white;
	}

	.file-item:hover {
		border-color: #007acc;
		background: #f8f9fa;
	}

	.file-item.selected {
		border-color: #007acc;
		background: #e3f2fd;
	}

	.file-checkbox {
		margin-right: 0.5rem;
	}

	.file-checkbox input[type="checkbox"] {
		width: auto;
		margin: 0;
		cursor: pointer;
	}

	.file-icon {
		font-size: 2rem;
		margin-right: 1rem;
	}

	.file-info {
		flex: 1;
	}

	.file-name {
		font-weight: 600;
		margin-bottom: 0.25rem;
		word-break: break-all;
	}

	.file-details {
		display: flex;
		gap: 1rem;
		font-size: 0.8rem;
		color: #666;
	}

	.file-size, .file-date {
		background: #f1f3f4;
		padding: 0.25rem 0.5rem;
		border-radius: 4px;
	}

	.empty-state {
		text-align: center;
		padding: 3rem;
		color: #666;
	}
</style>
