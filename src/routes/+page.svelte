<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";

	let sourcePath = "X:\\Innhealth\\Gait\\L\\test_video.mov";
	let destinationPath = "X:\\Innhealth\\Gait\\IH-0777-B\\test_video.mov";
	let result = "";
	let loading = false;

	async function moveFile() {
		try {
			loading = true;
			result = await invoke("move_file", { 
				sourcePath, 
				destinationPath 
			});
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}

	async function copyFile() {
		try {
			loading = true;
			result = await invoke("copy_file", { 
				sourcePath, 
				destinationPath 
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
			result = `File exists: ${exists}`;
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}

	async function getFileInfo() {
		try {
			loading = true;
			const info = await invoke("get_file_info", { filePath: sourcePath });
			result = `File info: ${JSON.stringify(info, null, 2)}`;
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}
</script>

<main class="container">
	<h1>File Operations</h1>

	<div class="form-group">
		<label for="source">Source Path:</label>
		<input id="source" bind:value={sourcePath} placeholder="Enter source file path" />
	</div>

	<div class="form-group">
		<label for="destination">Destination Path:</label>
		<input id="destination" bind:value={destinationPath} placeholder="Enter destination file path" />
	</div>

	<div class="buttons">
		<button on:click={checkFileExists} disabled={loading}>
			{loading ? 'Checking...' : 'Check File Exists'}
		</button>
		
		<button on:click={getFileInfo} disabled={loading}>
			{loading ? 'Loading...' : 'Get File Info'}
		</button>
		
		<button on:click={moveFile} disabled={loading}>
			{loading ? 'Moving...' : 'Move File'}
		</button>
		
		<button on:click={copyFile} disabled={loading}>
			{loading ? 'Copying...' : 'Copy File'}
		</button>
	</div>

	{#if result}
		<div class="result">
			<pre>{result}</pre>
		</div>
	{/if}
</main>

<style>
	.container {
		max-width: 800px;
		margin: 0 auto;
		padding: 2rem;
	}

	.form-group {
		margin-bottom: 1rem;
	}

	label {
		display: block;
		margin-bottom: 0.5rem;
		font-weight: bold;
	}

	input {
		width: 100%;
		padding: 0.5rem;
		border: 1px solid #ccc;
		border-radius: 4px;
		font-family: monospace;
	}

	.buttons {
		margin: 2rem 0;
		display: flex;
		gap: 1rem;
		flex-wrap: wrap;
	}

	button {
		padding: 0.5rem 1rem;
		background: #007acc;
		color: white;
		border: none;
		border-radius: 4px;
		cursor: pointer;
	}

	button:hover:not(:disabled) {
		background: #005a9e;
	}

	button:disabled {
		background: #ccc;
		cursor: not-allowed;
	}

	.result {
		margin-top: 2rem;
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
</style>
