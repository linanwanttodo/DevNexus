import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/**
 * Download progress utilities
 */
export const DownloadProgress = {
  formatSpeed: (speed) => {
    if (speed < 1024) {
      return `${speed.toFixed(1)} B/s`;
    } else if (speed < 1024 * 1024) {
      return `${(speed / 1024).toFixed(1)} KB/s`;
    } else if (speed < 1024 * 1024 * 1024) {
      return `${(speed / (1024 * 1024)).toFixed(1)} MB/s`;
    } else {
      return `${(speed / (1024 * 1024 * 1024)).toFixed(1)} GB/s`;
    }
  },

  formatSize: (size) => {
    if (size < 1024) {
      return `${size} B`;
    } else if (size < 1024 * 1024) {
      return `${(size / 1024).toFixed(1)} KB`;
    } else if (size < 1024 * 1024 * 1024) {
      return `${(size / (1024 * 1024)).toFixed(1)} MB`;
    } else {
      return `${(size / (1024 * 1024 * 1024)).toFixed(1)} GB`;
    }
  },

  formatEta: (seconds) => {
    if (!seconds) return '--:--';
    if (seconds < 60) {
      return `${seconds}s`;
    } else if (seconds < 3600) {
      const mins = Math.floor(seconds / 60);
      const secs = seconds % 60;
      return `${mins}m ${secs}s`;
    } else {
      const hours = Math.floor(seconds / 3600);
      const mins = Math.floor((seconds % 3600) / 60);
      return `${hours}h ${mins}m`;
    }
  }
};

/**
 * Download API functions
 */
export const downloadApi = {
  createDownload: async (url, savePath = null) => {
    return await invoke('create_download', { url, savePath });
  },

  startDownload: async (taskId) => {
    return await invoke('start_download', { taskId });
  },

  pauseDownload: async (taskId) => {
    return await invoke('pause_download', { taskId });
  },

  resumeDownload: async (taskId) => {
    return await invoke('resume_download', { taskId });
  },

  cancelDownload: async (taskId) => {
    return await invoke('cancel_download', { taskId });
  },

  deleteDownload: async (taskId, deleteFile = false) => {
    return await invoke('delete_download', { taskId, deleteFile });
  },

  getDownloads: async () => {
    return await invoke('get_downloads');
  },

  getDownload: async (taskId) => {
    return await invoke('get_download', { taskId });
  },

  getDownloadConfig: async () => {
    return await invoke('get_download_config');
  },

  setDownloadConfig: async (config) => {
    return await invoke('set_download_config', { config });
  },

  setupProgressListener: (callback) => {
    return listen('download-progress', (event) => {
      callback(event.payload);
    });
  }
};