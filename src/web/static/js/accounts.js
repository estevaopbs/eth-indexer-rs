// Accounts Page JavaScript

const API_BASE = "/api";
let currentPage = 1;
let perPage = 20;
let currentSort = "balance";
let sortOrder = "desc";
let isLoading = false;

// Format number with commas
function formatNumber(num) {
  return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

// Format ETH balance
function formatEthBalance(balance) {
  if (!balance || balance === "0") return "0 ETH";
  
  // Convert from Wei to ETH (assuming balance is in Wei)
  const eth = parseFloat(balance) / Math.pow(10, 18);
  
  if (eth < 0.001) {
    return "<0.001 ETH";
  } else if (eth < 1) {
    return eth.toFixed(4) + " ETH";
  } else if (eth < 1000) {
    return eth.toFixed(2) + " ETH";
  } else {
    return formatNumber(Math.floor(eth)) + " ETH";
  }
}

// Truncate address
function truncateAddress(address, length = 8) {
  if (!address) return "";
  return `${address.substring(0, length)}...${address.substring(address.length - length)}`;
}

// Format timestamp
function formatTimestamp(timestamp) {
  if (!timestamp) return "Never";
  return new Date(timestamp * 1000).toLocaleDateString();
}

// Format block number for first seen / last activity
function formatBlockNumber(blockNumber) {
  if (!blockNumber) return "Never";
  return formatNumber(blockNumber);
}

// Create block link for first seen / last activity
function createBlockLink(blockNumber) {
  if (!blockNumber) return "Never";
  return `<a href="/block-detail.html?number=${blockNumber}" class="text-blue-600 hover:text-blue-900 font-mono text-sm">${formatNumber(blockNumber)}</a>`;
}

// Get account type badge
function getAccountTypeBadge(type) {
  const typeMap = {
    'eoa': { label: 'EOA', class: 'bg-blue-100 text-blue-800' },
    'contract': { label: 'Contract', class: 'bg-purple-100 text-purple-800' },
    'unknown': { label: 'Unknown', class: 'bg-gray-100 text-gray-800' }
  };
  
  const accountType = typeMap[type] || typeMap['unknown'];
  
  return `<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${accountType.class}">
    ${accountType.label}
  </span>`;
}

// Load accounts from API
async function loadAccounts(page = 1, per_page = 20, sort = currentSort, order = sortOrder) {
  if (isLoading) return;
  isLoading = true;
  
  showLoading();
  hideError();
  
  try {
    const response = await fetch(`${API_BASE}/accounts?page=${page}&per_page=${per_page}&sort=${sort}&order=${order}`);
    
    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }
    
    const data = await response.json();
    displayAccounts(data.accounts || []);
    updatePagination(page, data.has_next || false);
    
  } catch (error) {
    console.error("Error loading accounts:", error);
    showError();
  } finally {
    isLoading = false;
    hideLoading();
  }
}

// Display accounts in table
function displayAccounts(accounts) {
  const tableBody = document.getElementById("accounts-table");
  tableBody.innerHTML = "";
  
  if (accounts.length === 0) {
    tableBody.innerHTML = `
      <tr>
        <td colspan="6" class="px-6 py-4 text-center text-gray-500">
          No accounts found
        </td>
      </tr>
    `;
    return;
  }
  
  accounts.forEach(account => {
    const row = document.createElement("tr");
    row.className = "hover:bg-gray-50";
    row.innerHTML = `
      <td class="px-6 py-4 whitespace-nowrap">
        <a href="#" onclick="viewAccount('${account.address}')" class="text-blue-600 hover:text-blue-900 font-mono text-sm">
          ${truncateAddress(account.address)}
        </a>
      </td>
      <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
        ${formatEthBalance(account.balance)}
      </td>
      <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
        ${formatNumber(account.transaction_count || 0)}
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        ${getAccountTypeBadge(account.account_type || 'unknown')}
      </td>
      <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
        ${createBlockLink(account.first_seen)}
      </td>
      <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
        ${createBlockLink(account.last_activity)}
      </td>
    `;
    tableBody.appendChild(row);
  });
}

// View individual account details
function viewAccount(address) {
  window.location.href = `/account-detail.html?address=${address}`;
}

// Handle sorting
function handleSort(sortBy) {
  if (currentSort === sortBy) {
    // Toggle sort order
    sortOrder = sortOrder === "desc" ? "asc" : "desc";
  } else {
    // New sort column
    currentSort = sortBy;
    sortOrder = "desc";
  }
  
  // Update sort indicators
  updateSortIndicators();
  
  // Reload data
  currentPage = 1;
  loadAccounts(currentPage, perPage, currentSort, sortOrder);
}

// Update sort indicators in table headers
function updateSortIndicators() {
  // Clear all indicators
  document.querySelectorAll('.sort-indicator').forEach(indicator => {
    indicator.innerHTML = '↕️';
  });
  
  // Set current sort indicator
  const indicator = document.querySelector(`[data-sort="${currentSort}"] .sort-indicator`);
  if (indicator) {
    indicator.innerHTML = sortOrder === "desc" ? "↓" : "↑";
  }
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
  document.getElementById("accounts-table").parentElement.parentElement.classList.add("opacity-50");
}

// Hide loading state
function hideLoading() {
  document.getElementById("loading-state").classList.add("hidden");
  document.getElementById("accounts-table").parentElement.parentElement.classList.remove("opacity-50");
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
      // Check if it's an Ethereum address (starts with 0x and is 42 chars)
      if (query.startsWith("0x") && query.length === 42 && /^0x[a-fA-F0-9]{40}$/.test(query)) {
        viewAccount(query);
      } else if (query.startsWith("0x") && query.length === 66 && /^0x[a-fA-F0-9]{64}$/.test(query)) {
        // Transaction hash - redirect to transaction detail
        window.location.href = `/transaction-detail.html?hash=${query}`;
      } else {
        // Try as block number
        const blockNumber = parseInt(query);
        if (!isNaN(blockNumber) && blockNumber >= 0 && blockNumber.toString() === query) {
          window.location.href = `/block-detail.html?number=${blockNumber}`;
        } else {
          // Use global search for other queries
          window.location.href = `/search.html?q=${encodeURIComponent(query)}`;
        }
      }
    }
  }
}

// Apply filters
function applyFilters() {
  const typeFilter = document.getElementById("type-filter").value;
  
  // Build filter parameters
  const filterParams = new URLSearchParams({
    page: '1', // Reset to first page when filtering
    per_page: perPage.toString(),
    sort: currentSort,
    order: sortOrder
  });
  
  if (typeFilter && typeFilter !== 'all') {
    filterParams.append('account_type', typeFilter);
  }
  
  // Load filtered accounts
  loadFilteredAccounts(filterParams);
}

// Load accounts with filters
async function loadFilteredAccounts(filterParams) {
  if (isLoading) return;
  isLoading = true;
  
  showLoading();
  hideError();
  
  try {
    const response = await fetch(`${API_BASE}/accounts/filtered?${filterParams.toString()}`);
    
    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }
    
    const data = await response.json();
    displayAccounts(data.accounts || []);
    updatePagination(parseInt(filterParams.get('page') || '1'), data.pagination?.has_next || false);
    
    // Update current page to reflect filters
    currentPage = parseInt(filterParams.get('page') || '1');
    
  } catch (error) {
    console.error("Error loading filtered accounts:", error);
    showError();
  } finally {
    isLoading = false;
    hideLoading();
  }
}

// Initialize page
document.addEventListener("DOMContentLoaded", function() {
  // Load initial data
  loadAccounts();
  
  // Setup sort indicators
  updateSortIndicators();
  
  // Setup event listeners
  document.getElementById("per-page-select").addEventListener("change", function() {
    perPage = parseInt(this.value);
    currentPage = 1;
    loadAccounts(currentPage, perPage, currentSort, sortOrder);
  });
  
  // Setup sort dropdown listener
  document.getElementById("sort-select").addEventListener("change", function() {
    const sortValue = this.value;
    const [sort, order] = sortValue.split('_');
    currentSort = sort;
    sortOrder = order;
    currentPage = 1;
    loadAccounts(currentPage, perPage, currentSort, sortOrder);
  });
  
  document.getElementById("prev-page").addEventListener("click", function() {
    if (currentPage > 1) {
      loadAccounts(currentPage - 1, perPage, currentSort, sortOrder);
    }
  });
  
  document.getElementById("next-page").addEventListener("click", function() {
    loadAccounts(currentPage + 1, perPage, currentSort, sortOrder);
  });
  
  document.getElementById("retry-btn").addEventListener("click", function() {
    loadAccounts(currentPage, perPage, currentSort, sortOrder);
  });
  
  document.getElementById("apply-filters").addEventListener("click", applyFilters);
  
  // Setup sort click handlers
  document.querySelectorAll('[data-sort]').forEach(header => {
    header.addEventListener('click', function() {
      const sortBy = this.getAttribute('data-sort');
      handleSort(sortBy);
    });
  });
});
