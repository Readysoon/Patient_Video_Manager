<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { onMount } from "svelte";

	interface FileInfo {
		name: string;
		path: string;
		is_dir: boolean;
		size?: number;
		modified?: number;
		thumbnails?: string[]; // Base64 encoded frame data
		thumbnailError?: boolean;
		isGeneratingThumbnail?: boolean;
		currentFrameIndex?: number;
	}

	let sourcePath = "X:\\Innhealth\\Gait\\L";
	let destinationPath = "X:\\Innhealth\\Gait\\IH-0777-B";
	let selectedSubfolder = "Gait-4K"; // Default selection
	let selectedSide = "L"; // Default side selection
	let files: FileInfo[] = [];
	let result = "";
	let loading = false;
	let selectedFiles: Set<string> = new Set(); // Track multiple selected files by path
	let frameIntervals = new Map<string, number>();

	const subfolderOptions = [
		"Calibration-Posture",
		"Gait-4K", 
		"Gait-720p",
		"Sitting",
		"Timedupandgo3m"
	];

	const sideOptions = ["L", "R"];

	async function listDirectory() {
		try {
			loading = true;
			result = "";
			const rawFiles = await invoke("list_directory", { dirPath: sourcePath }) as FileInfo[];
			
			// Process files and generate thumbnails for video files
			files = [];
			for (const file of rawFiles) {
				const fileInfo: FileInfo = {
					...file,
					thumbnailError: false,
					isGeneratingThumbnail: false,
					currentFrameIndex: 0
				};

				// Generate thumbnails for video files
				if (!file.is_dir && isVideoFile(file.name)) {
					fileInfo.isGeneratingThumbnail = true;
					files.push(fileInfo);
					
					// Generate thumbnails asynchronously
					generateVideoThumbnails(file.path, fileInfo.name).then(thumbnails => {
						files = files.map(f => 
							f.path === file.path 
								? { ...f, thumbnails, isGeneratingThumbnail: false, currentFrameIndex: 0 }
								: f
						);
						
						// Start frame cycling for this video
						startFrameCycling(file.path);
					}).catch(error => {
						console.warn('Failed to generate thumbnails for', file.name, error);
						files = files.map(f => 
							f.path === file.path 
								? { ...f, thumbnailError: true, isGeneratingThumbnail: false }
								: f
						);
					});
				} else {
					files.push(fileInfo);
				}
			}
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}

	async function generateVideoThumbnails(videoPath: string, fileName: string): Promise<string[]> {
		return new Promise(async (resolve, reject) => {
			try {
				// Read the video file as base64 from the backend
				const base64Data = await invoke("read_video_file", { filePath: videoPath });
				
				// Create a blob URL from the base64 data
				const binaryString = atob(base64Data as string);
				const bytes = new Uint8Array(binaryString.length);
				for (let i = 0; i < binaryString.length; i++) {
					bytes[i] = binaryString.charCodeAt(i);
				}
				const blob = new Blob([bytes], { type: 'video/mp4' });
				const videoUrl = URL.createObjectURL(blob);
				
				const video = document.createElement('video');
				const canvas = document.createElement('canvas');
				const ctx = canvas.getContext('2d');
				
				if (!ctx) {
					URL.revokeObjectURL(videoUrl);
					reject(new Error('Canvas not supported'));
					return;
				}

				const cleanup = () => {
					URL.revokeObjectURL(videoUrl);
					video.remove();
				};

				const timeout = setTimeout(() => {
					cleanup();
					reject(new Error('Timeout'));
				}, 30000); // 30 second timeout

				const thumbnails: string[] = [];
				let currentFrameIndex = 0;
				const framePercentages = [10, 20, 30, 40, 50, 60, 70, 80, 90, 95]; // 10 frames at different percentages

				const captureFrame = () => {
					try {
						ctx.drawImage(video, 0, 0);
						const dataURL = canvas.toDataURL('image/jpeg', 0.6);
						thumbnails.push(dataURL);
						
						currentFrameIndex++;
						
						if (currentFrameIndex < framePercentages.length) {
							// Seek to next frame
							const percentage = framePercentages[currentFrameIndex] / 100;
							video.currentTime = video.duration * percentage;
						} else {
							// All frames captured
							clearTimeout(timeout);
							cleanup();
							resolve(thumbnails);
						}
					} catch (error) {
						clearTimeout(timeout);
						cleanup();
						reject(error);
					}
				};

				video.addEventListener('loadedmetadata', () => {
					canvas.width = video.videoWidth;
					canvas.height = video.videoHeight;
					
					// Start with first frame at 10%
					const percentage = framePercentages[0] / 100;
					video.currentTime = video.duration * percentage;
				});

				video.addEventListener('seeked', captureFrame);

				video.addEventListener('error', () => {
					clearTimeout(timeout);
					cleanup();
					reject(new Error('Video load failed'));
				});

				// Important: Set properties before src
				video.muted = true;
				video.playsInline = true;
				video.preload = 'metadata';
				
				// Set source last
				video.src = videoUrl;
			} catch (error) {
				reject(error);
			}
		});
	}

	function startFrameCycling(videoPath: string) {
		// Clear any existing interval for this video
		const existingInterval = frameIntervals.get(videoPath);
		if (existingInterval) {
			clearInterval(existingInterval);
		}
		
		// Start new interval to cycle through frames
		const interval = setInterval(() => {
			files = files.map(f => {
				if (f.path === videoPath && f.thumbnails && f.thumbnails.length > 0) {
					const nextIndex = ((f.currentFrameIndex || 0) + 1) % f.thumbnails.length;
					return { ...f, currentFrameIndex: nextIndex };
				}
				return f;
			});
		}, 800); // Change frame every 800ms
		
		frameIntervals.set(videoPath, interval);
	}

	function stopFrameCycling(videoPath: string) {
		const interval = frameIntervals.get(videoPath);
		if (interval) {
			clearInterval(interval);
			frameIntervals.delete(videoPath);
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

			// Create full destination path with selected subfolder
			const fullDestinationPath = `${destinationPath}\\${selectedSubfolder}`;
			
			// Extract folder name from destination path
			const folderName = destinationPath.split('\\').pop() || '';

			for (const filePath of selectedFiles) {
				const file = files.find(f => f.path === filePath);
				if (file && !file.is_dir) {
					try {
						// Get file extension
						const fileExtension = file.name.substring(file.name.lastIndexOf('.'));
						
						// Create new filename with side prefix, folder name, and subfolder suffix
						const newFileName = `${selectedSide}-${folderName}-${selectedSubfolder}${fileExtension}`;
						const destPath = `${fullDestinationPath}\\${newFileName}`;
						
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
				result = `Successfully moved ${successCount} file(s) to ${fullDestinationPath}`;
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

			// Create full destination path with selected subfolder
			const fullDestinationPath = `${destinationPath}\\${selectedSubfolder}`;
			
			// Extract folder name from destination path
			const folderName = destinationPath.split('\\').pop() || '';

			for (const filePath of selectedFiles) {
				const file = files.find(f => f.path === filePath);
				if (file && !file.is_dir) {
					try {
						// Get file extension
						const fileExtension = file.name.substring(file.name.lastIndexOf('.'));
						
						// Create new filename with side prefix, folder name, and subfolder suffix
						const newFileName = `${selectedSide}-${folderName}-${selectedSubfolder}${fileExtension}`;
						const destPath = `${fullDestinationPath}\\${newFileName}`;
						
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
				result = `Successfully copied ${successCount} file(s) to ${fullDestinationPath}`;
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
			const fullDestinationPath = `${destinationPath}\\${selectedSubfolder}`;
			
			// Extract folder name from destination path
			const folderName = destinationPath.split('\\').pop() || '';
			
			// Get file extension
			const fileExtension = selectedFile.name.substring(selectedFile.name.lastIndexOf('.'));
			
			// Create new filename with side prefix, folder name, and subfolder suffix
			const newFileName = `${selectedSide}-${folderName}-${selectedSubfolder}${fileExtension}`;
			const destPath = `${fullDestinationPath}\\${newFileName}`;
			
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
			const fullDestinationPath = `${destinationPath}\\${selectedSubfolder}`;
			
			// Extract folder name from destination path
			const folderName = destinationPath.split('\\').pop() || '';
			
			// Get file extension
			const fileExtension = selectedFile.name.substring(selectedFile.name.lastIndexOf('.'));
			
			// Create new filename with side prefix, folder name, and subfolder suffix
			const newFileName = `${selectedSide}-${folderName}-${selectedSubfolder}${fileExtension}`;
			const destPath = `${fullDestinationPath}\\${newFileName}`;
			
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

	function isVideoFile(filename: string): boolean {
		const videoExtensions = ['.mp4', '.avi', '.mov', '.mkv', '.wmv', '.flv', '.webm', '.m4v'];
		const lowerFilename = filename.toLowerCase();
		return videoExtensions.some(ext => lowerFilename.endsWith(ext));
	}

	async function retryThumbnail(filePath: string) {
		const file = files.find(f => f.path === filePath);
		if (!file) return;

		// Reset states
		files = files.map(f => 
			f.path === filePath 
				? { ...f, thumbnailError: false, isGeneratingThumbnail: true }
				: f
		);

		try {
			const thumbnails = await generateVideoThumbnails(file.path, file.name);
			files = files.map(f => 
				f.path === filePath 
					? { ...f, thumbnails, isGeneratingThumbnail: false, currentFrameIndex: 0 }
					: f
			);
			
			// Start frame cycling for this video
			startFrameCycling(filePath);
		} catch (error) {
			console.warn('Retry failed for', file.name, error);
			files = files.map(f => 
				f.path === filePath 
					? { ...f, thumbnailError: true, isGeneratingThumbnail: false }
					: f
			);
		}
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

		<div class="form-group">
			<label>Side Selection:</label>
			<div class="side-options">
				{#each sideOptions as option}
					<label class="radio-option side-radio">
						<input 
							type="radio" 
							name="side" 
							value={option} 
							bind:group={selectedSide}
						/>
						<span class="radio-label">{option}</span>
					</label>
				{/each}
			</div>
		</div>

		<div class="form-group">
			<label>Destination Subfolder:</label>
			<div class="subfolder-options">
				{#each subfolderOptions as option}
					<label class="radio-option">
						<input 
							type="radio" 
							name="subfolder" 
							value={option} 
							bind:group={selectedSubfolder}
						/>
						<span class="radio-label">{option}</span>
					</label>
				{/each}
			</div>
		</div>

		<div class="destination-preview">
			<strong>Full Destination:</strong> {destinationPath}\{selectedSubfolder}
			<br>
			<strong>File Naming Pattern:</strong> {selectedSide}-{destinationPath.split('\\').pop()}-{selectedSubfolder}.MOV
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
						
						{#if !file.is_dir && isVideoFile(file.name)}
							<div class="video-preview-container">
								{#if file.thumbnails && file.thumbnails.length > 0}
									<div 
										class="animated-thumbnail"
										on:mouseenter={() => stopFrameCycling(file.path)}
										on:mouseleave={() => startFrameCycling(file.path)}
									>
										<img 
											src={file.thumbnails[file.currentFrameIndex || 0]} 
											alt="Video preview"
											class="thumbnail-frame"
										/>
										<div class="frame-indicator">
											{(file.currentFrameIndex || 0) + 1} / {file.thumbnails.length}
										</div>
									</div>
								{:else if file.thumbnailError}
									<div class="thumbnail error">
										<svg class="error-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
											<circle cx="12" cy="12" r="10"/>
											<line x1="15" y1="9" x2="9" y2="15"/>
											<line x1="9" y1="9" x2="15" y2="15"/>
										</svg>
										<span>Preview failed</span>
										<button class="retry-btn" on:click={() => retryThumbnail(file.path)}>Retry</button>
									</div>
								{:else if file.isGeneratingThumbnail}
									<div class="thumbnail loading">
										<div class="loading-spinner"></div>
										<span>Generating previews...</span>
									</div>
								{:else}
									<div class="thumbnail placeholder">
										<svg class="video-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
											<polygon points="23 7 16 12 23 17 23 7"/>
											<rect x="1" y="5" width="15" height="14" rx="2" ry="2"/>
										</svg>
										<span>Video</span>
									</div>
								{/if}
							</div>
						{:else}
							<div class="file-icon">
								{file.is_dir ? '📁' : '📄'}
							</div>
						{/if}
						
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

	.side-options {
		display: flex;
		gap: 1rem;
		margin-top: 0.5rem;
	}

	.side-radio {
		min-width: 80px;
		justify-content: center;
		font-size: 1.2rem;
		font-weight: bold;
	}

	.subfolder-options {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem;
		margin-top: 0.5rem;
	}

	.radio-option {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		cursor: pointer;
		padding: 0.5rem 1rem;
		background: white;
		border: 2px solid #e9ecef;
		border-radius: 6px;
		transition: all 0.2s;
		font-weight: normal;
		margin: 0;
	}

	.radio-option:hover {
		border-color: #007acc;
		background: #f8f9fa;
	}

	.radio-option input[type="radio"] {
		width: auto;
		margin: 0;
		cursor: pointer;
	}

	.radio-option input[type="radio"]:checked + .radio-label {
		color: #007acc;
		font-weight: 600;
	}

	.radio-option input[type="radio"]:checked {
		accent-color: #007acc;
	}

	.destination-preview {
		margin: 1rem 0;
		padding: 0.75rem;
		background: #e3f2fd;
		border-radius: 4px;
		border: 1px solid #007acc;
		color: #007acc;
		font-family: monospace;
		font-size: 0.9rem;
		line-height: 1.4;
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
		grid-template-columns: repeat(auto-fill, minmax(400px, 1fr));
		gap: 1rem;
		margin-bottom: 2rem;
	}

	.file-item {
		display: flex;
		align-items: flex-start;
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
		margin-top: 0.25rem;
	}

	.file-checkbox input[type="checkbox"] {
		width: auto;
		margin: 0;
		cursor: pointer;
	}

	.file-icon {
		font-size: 2rem;
		margin-right: 1rem;
		margin-top: 0.25rem;
	}

	.video-preview-container {
		width: 200px;
		height: 120px;
		margin-right: 1rem;
		position: relative;
		overflow: hidden;
		border-radius: 8px;
		border: 1px solid #ddd;
	}

	.animated-thumbnail {
		position: relative;
		width: 100%;
		height: 100%;
		border-radius: 8px;
		overflow: hidden;
		background: #000;
		cursor: pointer;
	}

	.thumbnail-frame {
		width: 100%;
		height: 100%;
		object-fit: cover;
		transition: opacity 0.3s ease;
	}

	.frame-indicator {
		position: absolute;
		bottom: 4px;
		right: 4px;
		background: rgba(0, 0, 0, 0.7);
		color: white;
		padding: 2px 6px;
		border-radius: 3px;
		font-size: 10px;
		font-weight: 500;
	}

	.thumbnail {
		width: 100%;
		height: 100%;
		object-fit: cover;
		border-radius: 8px;
		background: #000;
	}

	.thumbnail.loading, .thumbnail.error, .thumbnail.placeholder {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		background: #f0f0f0;
		color: #666;
		font-size: 12px;
		gap: 8px;
	}

	.thumbnail.error {
		background: #ffe6e6;
		color: #cc0000;
	}

	.thumbnail.placeholder {
		background: #f9f9f9;
		color: #999;
	}

	.error-icon, .video-icon {
		width: 24px;
		height: 24px;
	}

	.retry-btn {
		padding: 4px 8px;
		background: #4CAF50;
		color: white;
		border: none;
		border-radius: 4px;
		font-size: 10px;
		cursor: pointer;
		margin-top: 4px;
	}

	.retry-btn:hover {
		background: #45a049;
	}

	.loading-spinner {
		width: 20px;
		height: 20px;
		border: 2px solid #ddd;
		border-top: 2px solid #4CAF50;
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		0% { transform: rotate(0deg); }
		100% { transform: rotate(360deg); }
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
		margin-bottom: 0.5rem;
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
