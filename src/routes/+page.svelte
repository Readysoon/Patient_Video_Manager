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
	// Reactive default subfolder that updates when destinationPath changes
	$: selectedSubfolder = `${destinationPath.split('\\').pop() || ''}-Gait-4K`;
	let selectedSide = "L"; // Default side selection
	let files: FileInfo[] = [];
	let result = "";
	let loading = false;
	let selectedFile: string | null = null; // Track single selected file by path
	let frameIntervals = new Map<string, number>();
	let sortBy = "name"; // Default sort by name
	let sortDirection = "asc"; // Default sort direction

	// Thumbnail caching system
	let thumbnailCache = new Map<string, string[]>(); // Cache thumbnails by file hash
	let pendingFileOperations = new Map<string, { type: string; sourcePath: string; destinationPath: string; thumbnails?: string[] }>(); // Track files being moved/copied

	// Subfolder status tracking
	let subfolderStatus = new Map<string, { exists: boolean; hasL: boolean; hasR: boolean; fileCount: number }>();

	// Reactive subfolder options that update when destinationPath changes
	$: subfolderOptions = [
		`${destinationPath.split('\\').pop() || ''}-Calibration-Posture`,
		`${destinationPath.split('\\').pop() || ''}-Gait-4K`, 
		`${destinationPath.split('\\').pop() || ''}-Gait-720p`,
		`${destinationPath.split('\\').pop() || ''}-Sitting`,
		`${destinationPath.split('\\').pop() || ''}-Timedupandgo3m`
	];

	// Check subfolder status when destination path changes - REMOVED
	// Status will only update after move/copy operations

	const sideOptions = ["L", "R"];
	
	const sortOptions = [
		{ value: "name", label: "Name" },
		{ value: "modified", label: "Last Modified" },
		{ value: "size", label: "File Size" }
	];

	// Helper function to generate file hash
	async function generateFileHash(filePath: string): Promise<string> {
		try {
			const fileInfo = await invoke("get_file_info", { filePath }) as any;
			return `${fileInfo.size}-${fileInfo.modified}`;
		} catch (e) {
			// Fallback to path-based hash if metadata fails
			return filePath;
		}
	}

	// Smart refresh function that only regenerates thumbnails when necessary
	async function smartRefreshDirectory() {
		try {
			loading = true;
			result = "";
			const rawFiles = await invoke("list_directory", { dirPath: sourcePath }) as FileInfo[];
			
			// Process files intelligently
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
					const fileHash = await generateFileHash(file.path);
					
					// Check if we have cached thumbnails for this file
					if (thumbnailCache.has(fileHash)) {
						fileInfo.thumbnails = thumbnailCache.get(fileHash);
						fileInfo.isGeneratingThumbnail = false;
						files.push(fileInfo);
						
						// Start frame cycling for this video
						startFrameCycling(file.path);
					} else {
						// Check if this file was just moved and we have its thumbnails
						const pendingOp = pendingFileOperations.get(fileHash);
						if (pendingOp && pendingOp.type === 'move' && pendingOp.thumbnails) {
							fileInfo.thumbnails = pendingOp.thumbnails;
							fileInfo.isGeneratingThumbnail = false;
							files.push(fileInfo);
							
							// Cache the thumbnails for future use
							thumbnailCache.set(fileHash, pendingOp.thumbnails);
							// Clean up the pending operation
							pendingFileOperations.delete(fileHash);
							
							// Start frame cycling for this video
							startFrameCycling(file.path);
						} else {
							// Only generate thumbnails for genuinely new files
							fileInfo.isGeneratingThumbnail = true;
							files.push(fileInfo);
							
							// Generate thumbnails asynchronously
							generateVideoThumbnails(file.path, fileInfo.name).then(thumbnails => {
								if (thumbnails.length > 0) {
									// Cache the thumbnails
									thumbnailCache.set(fileHash, thumbnails);
									files = files.map(f => 
										f.path === file.path 
											? { ...f, thumbnails, isGeneratingThumbnail: false, currentFrameIndex: 0 }
											: f
									);
									
									// Start frame cycling for this video
									startFrameCycling(file.path);
								} else {
									// No thumbnails generated, show error state
									files = files.map(f => 
										f.path === file.path 
											? { ...f, thumbnailError: true, isGeneratingThumbnail: false }
											: f
									);
								}
							}).catch(error => {
								console.warn('Failed to generate thumbnails for', file.name, error);
								files = files.map(f => 
									f.path === file.path 
										? { ...f, thumbnailError: true, isGeneratingThumbnail: false }
										: f
								);
							});
						}
					}
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
		try {
			// Use the backend to generate thumbnails using ffmpeg
			const thumbnails = await invoke("generate_video_thumbnails", { filePath: videoPath }) as string[];
			
			// Convert base64 thumbnails to data URLs for display
			const validThumbnails = thumbnails.map(thumbnail => {
				if (thumbnail && thumbnail.trim() !== '') {
					return `data:image/jpeg;base64,${thumbnail}`;
				}
				return '';
			}).filter(thumbnail => thumbnail !== '');
			
			// If no valid thumbnails were generated, return empty array
			if (validThumbnails.length === 0) {
				console.warn('No valid thumbnails generated for:', fileName);
				return [];
			}
			
			return validThumbnails;
		} catch (error) {
			console.warn('Failed to generate thumbnails:', error);
			// Return empty array instead of throwing error
			return [];
		}
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

	async function moveSelectedFile() {
		if (!selectedFile) {
			result = "Please select a file first";
			return;
		}

		try {
			loading = true;
			const file = files.find(f => f.path === selectedFile);
			if (!file || file.is_dir) {
				result = "Please select a valid file";
				return;
			}

			// Create full destination path with selected subfolder
			const fullDestinationPath = `${destinationPath}\\${selectedSubfolder}`;
			
			// Get file extension
			const fileExtension = file.name.substring(file.name.lastIndexOf('.'));
			
			// Create new filename with side prefix and subfolder (which already includes folder name)
			const newFileName = `${selectedSide}-${selectedSubfolder}${fileExtension}`;
			const destPath = `${fullDestinationPath}\\${newFileName}`;
			
			await invoke("move_file", { 
				sourcePath: file.path, 
				destinationPath: destPath
			});
			
			// Clear selection after moving
			selectedFile = null;

			result = `Successfully moved ${file.name} to ${fullDestinationPath}`;

			// Refresh the list after moving
			await listDirectory();
			
			// Refresh subfolder status after moving
			await checkSubfolderStatus();
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}

	async function copySelectedFile() {
		if (!selectedFile) {
			result = "Please select a file first";
			return;
		}

		try {
			loading = true;
			const file = files.find(f => f.path === selectedFile);
			if (!file || file.is_dir) {
				result = "Please select a valid file";
				return;
			}

			// Create full destination path with selected subfolder
			const fullDestinationPath = `${destinationPath}\\${selectedSubfolder}`;
			
			// Get file extension
			const fileExtension = file.name.substring(file.name.lastIndexOf('.'));
			
			// Create new filename with side prefix and subfolder (which already includes folder name)
			const newFileName = `${selectedSide}-${selectedSubfolder}${fileExtension}`;
			const destPath = `${fullDestinationPath}\\${newFileName}`;
			
			await invoke("copy_file", { 
				sourcePath: file.path, 
				destinationPath: destPath
			});
			
			result = `Successfully copied ${file.name} to ${fullDestinationPath}`;
			
			// Refresh subfolder status after copying
			await checkSubfolderStatus();
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}

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
					const fileHash = await generateFileHash(file.path);
					
					// Check if we have cached thumbnails for this file
					if (thumbnailCache.has(fileHash)) {
						fileInfo.thumbnails = thumbnailCache.get(fileHash);
						fileInfo.isGeneratingThumbnail = false;
						files.push(fileInfo);
						
						// Start frame cycling for this video
						startFrameCycling(file.path);
					} else {
						fileInfo.isGeneratingThumbnail = true;
						files.push(fileInfo);
						
						// Generate thumbnails asynchronously
						generateVideoThumbnails(file.path, fileInfo.name).then(thumbnails => {
							if (thumbnails.length > 0) {
								// Cache the thumbnails
								thumbnailCache.set(fileHash, thumbnails);
								files = files.map(f => 
									f.path === file.path 
										? { ...f, thumbnails, isGeneratingThumbnail: false, currentFrameIndex: 0 }
										: f
								);
								
								// Start frame cycling for this video
								startFrameCycling(file.path);
							} else {
								// No thumbnails generated, show error state
								files = files.map(f => 
									f.path === file.path 
										? { ...f, thumbnailError: true, isGeneratingThumbnail: false }
										: f
								);
							}
						}).catch(error => {
							console.warn('Failed to generate thumbnails for', file.name, error);
							files = files.map(f => 
								f.path === file.path 
									? { ...f, thumbnailError: true, isGeneratingThumbnail: false }
									: f
							);
						});
					}
				} else {
					files.push(fileInfo);
				}
			}
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
		
		// Sort files after loading
		sortFiles();
	}

	async function moveFile() {
		if (!selectedFile) {
			result = "Please select a file first";
			return;
		}

		const file = files.find(f => f.path === selectedFile);
		if (!file) {
			result = "Please select a valid file";
			return;
		}

		try {
			loading = true;
			const fullDestinationPath = `${destinationPath}\\${selectedSubfolder}`;
			
			// Get file extension
			const fileExtension = file.name.substring(file.name.lastIndexOf('.'));
			
			// Create new filename with side prefix and subfolder (which already includes folder name)
			const newFileName = `${selectedSide}-${selectedSubfolder}${fileExtension}`;
			const destPath = `${fullDestinationPath}\\${newFileName}`;
			
			// Store the file's thumbnails before moving
			const existingThumbnails = file.thumbnails;
			const fileHash = await generateFileHash(file.path);
			
			// Mark this file as being moved
			if (existingThumbnails) {
				pendingFileOperations.set(fileHash, {
					type: 'move',
					sourcePath: file.path,
					destinationPath: destPath,
					thumbnails: existingThumbnails
				});
			}
			
			result = await invoke("move_file", { 
				sourcePath: file.path, 
				destinationPath: destPath
			});
			
			// Use smart refresh instead of full directory refresh
			await smartRefreshDirectory();
			
			// Refresh subfolder status after moving
			await checkSubfolderStatus();
			
			// Sort files after refresh
			sortFiles();
		} catch (e) {
			result = `Error: ${e}`;
		} finally {
			loading = false;
		}
	}

	async function copyFile() {
		if (!selectedFile) {
			result = "Please select a file first";
			return;
		}

		const file = files.find(f => f.path === selectedFile);
		if (!file) {
			result = "Please select a valid file";
			return;
		}

		try {
			loading = true;
			const fullDestinationPath = `${destinationPath}\\${selectedSubfolder}`;
			
			// Get file extension
			const fileExtension = file.name.substring(file.name.lastIndexOf('.'));
			
			// Create new filename with side prefix and subfolder (which already includes folder name)
			const newFileName = `${selectedSide}-${selectedSubfolder}${fileExtension}`;
			const destPath = `${fullDestinationPath}\\${newFileName}`;
			
			result = await invoke("copy_file", { 
				sourcePath: file.path, 
				destinationPath: destPath
			});
			
			// Refresh subfolder status after copying
			await checkSubfolderStatus();
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

	function selectFile(file: FileInfo) {
		selectedFile = file.path;
		result = `Selected: ${file.name}`;
	}

	function clearSelection() {
		selectedFile = null;
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

	function sortFiles() {
		if (sortBy === "name") {
			files = files.sort((a, b) => {
				const comparison = a.name.localeCompare(b.name);
				return sortDirection === "asc" ? comparison : -comparison;
			});
		} else if (sortBy === "modified") {
			files = files.sort((a, b) => {
				const aTime = a.modified || 0;
				const bTime = b.modified || 0;
				return sortDirection === "asc" ? aTime - bTime : bTime - aTime;
			});
		} else if (sortBy === "size") {
			files = files.sort((a, b) => {
				const aSize = a.size || 0;
				const bSize = b.size || 0;
				return sortDirection === "asc" ? aSize - bSize : bSize - aSize;
			});
		}
		files = files; // Trigger reactivity
	}

	function toggleSortDirection() {
		sortDirection = sortDirection === "asc" ? "desc" : "asc";
		sortFiles();
	}

	// Auto-load directory on mount - REMOVED
	// Directory will only load when "List Directory" button is clicked

	async function checkSubfolderStatus() {
		for (const subfolder of subfolderOptions) {
			try {
				const exists = await invoke("directory_exists", { dirPath: `${destinationPath}\\${subfolder}` }) as boolean;
				subfolderStatus.set(subfolder, { exists, hasL: false, hasR: false, fileCount: 0 });
				// Trigger reactivity by reassigning the Map
				subfolderStatus = new Map(subfolderStatus);

				if (exists) {
					// Check for L and R video files within the subfolder
					const files = await invoke("list_directory", { dirPath: `${destinationPath}\\${subfolder}` }) as FileInfo[];
					let fileCount = 0;
					let hasL = false;
					let hasR = false;
					
					console.log(`Checking subfolder: ${subfolder}`);
					console.log(`Files found:`, files.map(f => f.name));
					
					for (const file of files) {
						if (!file.is_dir && isVideoFile(file.name)) {
							fileCount++;
							console.log(`Video file found: ${file.name}`);
							
							// Check for files starting with L- or R- (the new naming pattern)
							const lowerName = file.name.toLowerCase();
							console.log(`Checking file: ${file.name} (lowercase: ${lowerName})`);
							
							if (lowerName.startsWith('l-')) {
								console.log(`Found L file: ${file.name}`);
								hasL = true;
							} else if (lowerName.startsWith('r-')) {
								console.log(`Found R file: ${file.name}`);
								hasR = true;
							}
						}
					}
					
					console.log(`Final status for ${subfolder}: fileCount=${fileCount}, hasL=${hasL}, hasR=${hasR}`);
					
					// Update file count and L/R status
					subfolderStatus.set(subfolder, { exists, hasL, hasR, fileCount });
					// Trigger reactivity by reassigning the Map
					subfolderStatus = new Map(subfolderStatus);
				}
			} catch (e) {
				console.warn(`Could not check status for subfolder ${subfolder}:`, e);
				subfolderStatus.set(subfolder, { exists: false, hasL: false, hasR: false, fileCount: 0 });
				// Trigger reactivity by reassigning the Map
				subfolderStatus = new Map(subfolderStatus);
			}
		}
	}
</script>

<main class="container">
	<h1>📁 Directory File Manager</h1>

	<h2>File Moving only possible after all thumbnails are loaded :)</h2>

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
					{@const status = subfolderStatus.get(option) || { exists: false, hasL: false, hasR: false, fileCount: 0 }}
					<label class="radio-option subfolder-option" class:exists={status.exists}>
						<input 
							type="radio" 
							name="subfolder" 
							value={option} 
							bind:group={selectedSubfolder}
						/>
						<span class="radio-label">{option}</span>
						<div class="subfolder-status">
							{#if status.exists}
								<span class="status-indicator exists">📁</span>
								<span class="status-indicator file-count">{status.fileCount}/2</span>
								{#if status.hasL}
									<span class="status-indicator has-l">L</span>
								{/if}
								{#if status.hasR}
									<span class="status-indicator has-r">R</span>
								{/if}
								{#if status.hasL && status.hasR}
									<span class="status-indicator complete">✅</span>
								{/if}
							{:else}
								<span class="status-indicator missing">❌</span>
								<span class="status-indicator file-count">0/2</span>
							{/if}
						</div>
					</label>
				{/each}
			</div>
		</div>

		<div class="form-group">
			<label>Sort Files By:</label>
			<div class="sort-options">
				{#each sortOptions as option}
					<div class="sort-option-container">
						<label class="radio-option">
							<input 
								type="radio" 
								name="sort" 
								value={option.value} 
								bind:group={sortBy}
								on:change={sortFiles}
							/>
							<span class="radio-label">{option.label}</span>
						</label>
						{#if sortBy === option.value}
							<button 
								class="direction-toggle" 
								on:click={toggleSortDirection}
								title="Toggle sort direction"
							>
								{sortDirection === "asc" ? "↑" : "↓"}
							</button>
						{/if}
					</div>
				{/each}
			</div>
		</div>

		<div class="destination-preview">
			<strong>Full Destination:</strong> {destinationPath}\{selectedSubfolder}
			<br>
			<strong>File Naming Pattern:</strong> {selectedSide}-{selectedSubfolder}.MOV
		</div>

		<div class="buttons">
			<button on:click={checkFileExists} disabled={loading}>
				{loading ? 'Checking...' : 'Check Directory'}
			</button>
			
			<button on:click={listDirectory} disabled={loading}>
				{loading ? 'Loading...' : 'List Directory'}
			</button>

			<button on:click={checkSubfolderStatus} disabled={loading}>
				{loading ? 'Checking...' : 'Refresh Subfolder Status'}
			</button>

			<button on:click={moveSelectedFile} disabled={loading || !selectedFile}>
				{loading ? 'Moving...' : 'Move Selected File'}
			</button>

			<button on:click={copySelectedFile} disabled={loading || !selectedFile}>
				{loading ? 'Copying...' : 'Copy Selected File'}
			</button>

			<button on:click={clearSelection} disabled={loading || !selectedFile}>
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
				Selected: {selectedFile ? files.find(f => f.path === selectedFile)?.name || 'None' : 'None'}
			</div>
			<div class="file-grid">
				{#each files as file}
					<div class="file-item" class:selected={selectedFile === file.path} on:click={() => selectFile(file)}>
						<div class="file-radio">
							<input type="radio" name="fileSelection" value={file.path} checked={selectedFile === file.path} on:change={() => selectFile(file)} />
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
										<span>FFmpeg not installed</span>
										<span class="error-detail">Install FFmpeg for video previews</span>
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

	h2 {
		text-align: center;
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

	.subfolder-option {
		position: relative;
		min-width: 200px;
	}

	.subfolder-status {
		display: flex;
		gap: 0.25rem;
		margin-left: 0.5rem;
		align-items: center;
	}

	.status-indicator {
		font-size: 0.8rem;
		padding: 0.1rem 0.3rem;
		border-radius: 3px;
		font-weight: bold;
	}

	.status-indicator.exists {
		background: #e3f2fd;
		color: #1976d2;
	}

	.status-indicator.has-l {
		background: #e8f5e8;
		color: #2e7d32;
	}

	.status-indicator.has-r {
		background: #fff3e0;
		color: #f57c00;
	}

	.status-indicator.complete {
		background: #e8f5e8;
		color: #2e7d32;
		font-size: 1rem;
	}

	.status-indicator.missing {
		background: #ffebee;
		color: #c62828;
	}

	.status-indicator.file-count {
		background: #f5f5f5;
		color: #666;
		font-size: 0.75rem;
		font-weight: 600;
	}

	.subfolder-option.exists {
		border-color: #4caf50;
		background: #f1f8e9;
	}

	.sort-options {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem;
		margin-top: 0.5rem;
	}

	.sort-option-container {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.direction-toggle {
		padding: 0.25rem 0.5rem;
		background: #007acc;
		color: white;
		border: none;
		border-radius: 4px;
		cursor: pointer;
		font-size: 0.8rem;
		font-weight: bold;
		transition: background-color 0.2s;
		min-width: 30px;
		height: 30px;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.direction-toggle:hover {
		background: #005a9e;
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

	.file-radio {
		margin-right: 0.5rem;
		margin-top: 0.25rem;
	}

	.file-radio input[type="radio"] {
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
	
	.error-detail {
		font-size: 10px;
		opacity: 0.8;
		text-align: center;
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
