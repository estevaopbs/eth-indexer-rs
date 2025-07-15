// Transactions Page JavaScript

const API_BASE = "/api";
let currentPage = 1;
let perPage = 20;
let isLoading = false;

// Format number with commas
function formatNumber(num) {
  return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

// Format ETH value
function formatEthValue(value) {
  if (!value || value === "0") return "0 ETH";
  
  // Convert from Wei to ETH (assuming value is in Wei)
  const eth = parseFloat(value) / Math.pow(10, 18);
  
  if (eth < 0.001) {
    return "<0.001 ETH";
  } else if (eth < 1) {
    return eth.toFixed(4) + " ETH";
  } else {
    return eth.toFixed(2) + " ETH";
  }
}

// Truncate address
function truncateAddress(address, length = 6) {
  if (!address) return "";
  return `${address.substring(0, length)}...${address.substring(address.length - length)}`;
}

// Truncate hash
function truncateHash(hash, length = 8) {
  if (!hash) return "";
  return `${hash.substring(0, length)}...${hash.substring(hash.length - length)}`;
}

// Get status badge
function getStatusBadge(status) {
  const isSuccess = status === "success" || status === true || status === 1;
  
  if (isSuccess) {
    return '<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">Success</span>';
  } else {
    return '<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-800">Failed</span>';
  }
}

// Load transactions from API
async function loadTransactions(page = 1, per_page = 20) {
  if (isLoading) return;
  isLoading = true;
  
  showLoading();
  hideError();
  
  try {
    const response = await fetch(`${API_BASE}/transactions?page=${page}&per_page=${per_page}`);
    
    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }
    
    const data = await response.json();
    displayTransactions(data.transactions || []);
    updatePagination(page, data.pagination?.has_next || false);
    
  } catch (error) {
    console.error("Error loading transactions:", error);
    showError();
  } finally {
    isLoading = false;
    hideLoading();
  }
}

// Display transactions in table
function displayTransactions(transactions) {
  const tableBody = document.getElementById("transactions-table");
  tableBody.innerHTML = "";
  
  if (transactions.length === 0) {
    tableBody.innerHTML = `
      <tr>
        <td colspan="7" class="px-6 py-4 text-center text-gray-500">
          No transactions found
        </td>
      </tr>
    `;
    return;
  }
  
  transactions.forEach(tx => {
    const row = document.createElement("tr");
    row.className = "hover:bg-gray-50";
    row.innerHTML = `
      <td class="px-6 py-4 whitespace-nowrap">
        <a href="#" onclick="viewTransaction('${tx.hash}')" class="text-blue-600 hover:text-blue-900 font-mono text-sm">
          ${truncateHash(tx.hash)}
        </a>
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        <a href="#" onclick="viewBlock(${tx.block_number})" class="text-blue-600 hover:text-blue-900 font-mono text-sm">
          ${formatNumber(tx.block_number)}
        </a>
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        <a href="#" onclick="viewAddress('${tx.from_address}')" class="text-blue-600 hover:text-blue-900 font-mono text-sm">
          ${truncateAddress(tx.from_address)}
        </a>
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        ${tx.to_address ? 
          `<a href="#" onclick="viewAddress('${tx.to_address}')" class="text-blue-600 hover:text-blue-900 font-mono text-sm">${truncateAddress(tx.to_address)}</a>` :
          '<span class="text-gray-400">Contract Creation</span>'
        }
      </td>
      <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
        ${formatEthValue(tx.value)}
      </td>
      <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
        ${formatNumber(tx.gas_used || 0)}
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        ${getStatusBadge(tx.status)}
      </td>
    `;
    tableBody.appendChild(row);
  });
}

// View individual transaction details
function viewTransaction(txHash) {
  window.location.href = `/transaction-detail.html?hash=${txHash}`;
}

// View block details
function viewBlock(blockNumber) {
  window.location.href = `/block-detail.html?number=${blockNumber}`;
}

// View address details
function viewAddress(address) {
  window.location.href = `/account-detail.html?address=${address}`;
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
  document.getElementById("transactions-table").parentElement.parentElement.classList.add("opacity-50");
}

// Hide loading state
function hideLoading() {
  document.getElementById("loading-state").classList.add("hidden");
  document.getElementById("transactions-table").parentElement.parentElement.classList.remove("opacity-50");
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
      // Check if it's a transaction hash (starts with 0x and is 66 chars)
      if (query.startsWith("0x") && query.length === 66 && /^0x[a-fA-F0-9]{64}$/.test(query)) {
        viewTransaction(query);
      } else if (query.startsWith("0x") && query.length === 42 && /^0x[a-fA-F0-9]{40}$/.test(query)) {
        // It's an address
        viewAddress(query);
      } else {
        // Try as block number
        const blockNumber = parseInt(query);
        if (!isNaN(blockNumber) && blockNumber >= 0 && blockNumber.toString() === query) {
          viewBlock(blockNumber);
        } else {
          // Use global search for other queries
          window.location.href = `/search.html?q=${encodeURIComponent(query)}`;
        }
      }
    }
  }
}

// Initialize page
document.addEventListener("DOMContentLoaded", function() {
  // Load initial data
  loadTransactions();
  
  // Setup event listeners
  document.getElementById("per-page-select").addEventListener("change", function() {
    perPage = parseInt(this.value);
    currentPage = 1;
    loadTransactions(currentPage, perPage);
  });
  
  document.getElementById("prev-page").addEventListener("click", function() {
    if (currentPage > 1) {
      loadTransactions(currentPage - 1, perPage);
    }
  });
  
  document.getElementById("next-page").addEventListener("click", function() {
    loadTransactions(currentPage + 1, perPage);
  });
  
  document.getElementById("retry-btn").addEventListener("click", function() {
    loadTransactions(currentPage, perPage);
  });
});
