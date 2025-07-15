// Blocks Page JavaScript

const API_BASE = "/api";
let currentPage = 1;
let perPage = 20;
let isLoading = false;

// Format number with commas
function formatNumber(num) {
  return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

// Format timestamp to readable date
function formatTimestamp(timestamp) {
  return new Date(timestamp * 1000).toLocaleString();
}

// Truncate hash
function truncateHash(hash, length = 10) {
  if (!hash) return "";
  return `${hash.substring(0, length)}...${hash.substring(hash.length - length)}`;
}

// Load blocks from API
async function loadBlocks(page = 1, per_page = 20) {
  if (isLoading) return;
  isLoading = true;
  
  showLoading();
  hideError();
  
  try {
    const response = await fetch(`${API_BASE}/blocks?page=${page}&per_page=${per_page}`);
    
    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }
    
    const data = await response.json();
    displayBlocks(data.blocks || []);
    updatePagination(page, data.has_next || false);
    
  } catch (error) {
    console.error("Error loading blocks:", error);
    showError();
  } finally {
    isLoading = false;
    hideLoading();
  }
}

// Display blocks in table
function displayBlocks(blocks) {
  const tableBody = document.getElementById("blocks-table");
  tableBody.innerHTML = "";
  
  if (blocks.length === 0) {
    tableBody.innerHTML = `
      <tr>
        <td colspan="6" class="px-6 py-4 text-center text-gray-500">
          No blocks found
        </td>
      </tr>
    `;
    return;
  }
  
  blocks.forEach(block => {
    const row = document.createElement("tr");
    row.className = "hover:bg-gray-50";
    row.innerHTML = `
      <td class="px-6 py-4 whitespace-nowrap">
        <a href="#" onclick="viewBlock(${block.number})" class="text-blue-600 hover:text-blue-900 font-mono">
          ${formatNumber(block.number)}
        </a>
      </td>
      <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
        ${formatTimestamp(block.timestamp)}
      </td>
      <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
        ${formatNumber(block.transaction_count || 0)}
      </td>
      <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
        ${formatNumber(block.gas_used || 0)}
      </td>
      <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
        ${formatNumber(block.gas_limit || 0)}
      </td>
      <td class="px-6 py-4 whitespace-nowrap text-sm font-mono text-gray-500">
        ${truncateHash(block.hash)}
      </td>
    `;
    tableBody.appendChild(row);
  });
}

// View individual block details
function viewBlock(blockNumber) {
  window.location.href = `/block-detail.html?number=${blockNumber}`;
}

// Update pagination controls
function updatePagination(page, hasNext) {
  currentPage = page;
  
  document.getElementById("page-info").textContent = `Page ${page}`;
  document.getElementById("prev-page").disabled = page <= 1;
  document.getElementById("next-page").disabled = !hasNext;
}

// Show loading state
function showLoading() {
  document.getElementById("loading-state").classList.remove("hidden");
  document.getElementById("blocks-table").parentElement.parentElement.classList.add("opacity-50");
}

// Hide loading state
function hideLoading() {
  document.getElementById("loading-state").classList.add("hidden");
  document.getElementById("blocks-table").parentElement.parentElement.classList.remove("opacity-50");
}

// Show error state
function showError() {
  document.getElementById("error-state").classList.remove("hidden");
}

// Hide error state
function hideError() {
  document.getElementById("error-state").classList.add("hidden");
}

// Handle search
function handleSearchKeyPress(event) {
  if (event.key === "Enter") {
    const query = event.target.value.trim();
    if (query) {
      // Check if it's a block number
      const blockNumber = parseInt(query);
      if (!isNaN(blockNumber) && blockNumber >= 0 && blockNumber.toString() === query) {
        viewBlock(blockNumber);
      } else if (query.startsWith("0x") && query.length === 66) {
        // Transaction hash - redirect to transaction detail
        window.location.href = `/transaction-detail.html?hash=${query}`;
      } else if (query.startsWith("0x") && query.length === 42) {
        // Address - redirect to account detail
        window.location.href = `/account-detail.html?address=${query}`;
      } else {
        // Use global search for other queries
        window.location.href = `/search.html?q=${encodeURIComponent(query)}`;
      }
    }
  }
}

// Initialize page
document.addEventListener("DOMContentLoaded", function() {
  // Load initial data
  loadBlocks();
  
  // Setup event listeners
  document.getElementById("per-page-select").addEventListener("change", function() {
    perPage = parseInt(this.value);
    currentPage = 1;
    loadBlocks(currentPage, perPage);
  });
  
  document.getElementById("prev-page").addEventListener("click", function() {
    if (currentPage > 1) {
      loadBlocks(currentPage - 1, perPage);
    }
  });
  
  document.getElementById("next-page").addEventListener("click", function() {
    loadBlocks(currentPage + 1, perPage);
  });
  
  document.getElementById("retry-btn").addEventListener("click", function() {
    loadBlocks(currentPage, perPage);
  });
});
