// Search Page JavaScript

const API_BASE = "/api";
let isSearching = false;

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

// Format timestamp
function formatTimestamp(timestamp) {
  const date = new Date(timestamp * 1000);
  return date.toLocaleString();
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

// Get status badge
function getStatusBadge(status) {
  const isSuccess = status === "success" || status === true || status === 1;
  
  if (isSuccess) {
    return '<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">Success</span>';
  } else {
    return '<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-800">Failed</span>';
  }
}

// Determine search type based on query
function determineSearchType(query) {
  query = query.trim();
  
  if (!query) return null;
  
  // Check if it's a transaction hash (0x + 64 hex chars)
  if (query.startsWith("0x") && query.length === 66 && /^0x[a-fA-F0-9]{64}$/.test(query)) {
    return { type: 'transaction', value: query };
  }
  
  // Check if it's an address (0x + 40 hex chars)
  if (query.startsWith("0x") && query.length === 42 && /^0x[a-fA-F0-9]{40}$/.test(query)) {
    return { type: 'address', value: query };
  }
  
  // Check if it's a block number (numeric)
  const blockNumber = parseInt(query);
  if (!isNaN(blockNumber) && blockNumber >= 0 && blockNumber.toString() === query) {
    return { type: 'block', value: blockNumber };
  }
  
  // Check if it's a partial hash or address
  if (query.startsWith("0x") && query.length > 2) {
    // Could be partial hash or address
    return { type: 'partial_hex', value: query };
  }
  
  return { type: 'unknown', value: query };
}

// Perform intelligent search
async function performSearch(query) {
  if (isSearching) return;
  isSearching = true;
  
  showLoading();
  hideAllStates();
  
  try {
    const searchInfo = determineSearchType(query);
    
    if (!searchInfo) {
      showNoResults();
      return;
    }
    
    let results = null;
    
    switch (searchInfo.type) {
      case 'transaction':
        results = await searchTransaction(searchInfo.value);
        break;
      case 'address':
        results = await searchAddress(searchInfo.value);
        break;
      case 'block':
        results = await searchBlock(searchInfo.value);
        break;
      case 'partial_hex':
        results = await searchPartialHex(searchInfo.value);
        break;
      default:
        showNoResults();
        return;
    }
    
    if (results && results.found) {
      displayResults(results);
    } else {
      showNoResults();
    }
    
  } catch (error) {
    console.error("Search error:", error);
    showError(error.message);
  } finally {
    isSearching = false;
    hideLoading();
  }
}

// Search for transaction
async function searchTransaction(hash) {
  try {
    const response = await fetch(`${API_BASE}/transactions/${hash}`);
    
    if (response.ok) {
      const transaction = await response.json();
      return {
        found: true,
        type: 'transaction',
        data: transaction,
        redirect: `/transaction-detail.html?hash=${hash}`
      };
    } else if (response.status === 404) {
      return { found: false };
    } else {
      throw new Error(`API returned ${response.status}`);
    }
  } catch (error) {
    throw new Error(`Transaction search failed: ${error.message}`);
  }
}

// Search for address/account
async function searchAddress(address) {
  try {
    const response = await fetch(`${API_BASE}/accounts/${address}`);
    
    if (response.ok) {
      const account = await response.json();
      return {
        found: true,
        type: 'account',
        data: account,
        redirect: `/account-detail.html?address=${address}`
      };
    } else if (response.status === 404) {
      return { found: false };
    } else {
      throw new Error(`API returned ${response.status}`);
    }
  } catch (error) {
    throw new Error(`Address search failed: ${error.message}`);
  }
}

// Search for block
async function searchBlock(blockNumber) {
  try {
    const response = await fetch(`${API_BASE}/blocks/${blockNumber}`);
    
    if (response.ok) {
      const block = await response.json();
      return {
        found: true,
        type: 'block',
        data: block,
        redirect: `/block-detail.html?number=${blockNumber}`
      };
    } else if (response.status === 404) {
      return { found: false };
    } else {
      throw new Error(`API returned ${response.status}`);
    }
  } catch (error) {
    throw new Error(`Block search failed: ${error.message}`);
  }
}

// Search for partial hex (could be partial hash or address)
async function searchPartialHex(partial) {
  // For now, suggest user to enter complete hash/address
  return { found: false };
}

// Display search results
function displayResults(results) {
  const container = document.getElementById("search-results");
  
  if (results.redirect) {
    // Direct redirect for exact matches
    setTimeout(() => {
      window.location.href = results.redirect;
    }, 500);
    
    container.innerHTML = `
      <div class="text-center">
        <div class="inline-flex items-center px-4 py-2 font-semibold leading-6 text-sm shadow rounded-md text-white bg-green-500">
          <svg class="animate-spin -ml-1 mr-3 h-5 w-5 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
          </svg>
          Found! Redirecting...
        </div>
        <p class="mt-2 text-sm text-gray-600">Taking you to the ${results.type} details page...</p>
      </div>
    `;
  } else {
    // Show results inline
    container.innerHTML = generateResultsHTML(results);
  }
  
  container.classList.remove("hidden");
}

// Generate HTML for results
function generateResultsHTML(results) {
  switch (results.type) {
    case 'transaction':
      return generateTransactionHTML(results.data);
    case 'account':
      return generateAccountHTML(results.data);
    case 'block':
      return generateBlockHTML(results.data);
    default:
      return '<p>Unknown result type</p>';
  }
}

// Generate transaction result HTML
function generateTransactionHTML(tx) {
  return `
    <div class="bg-white shadow rounded-lg p-6">
      <h3 class="text-lg font-medium text-gray-900 mb-4">Transaction Found</h3>
      <div class="space-y-3">
        <div class="flex justify-between">
          <span class="text-sm font-medium text-gray-500">Hash:</span>
          <span class="text-sm text-gray-900 font-mono">${truncateHash(tx.hash, 12)}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-sm font-medium text-gray-500">Block:</span>
          <span class="text-sm text-gray-900">${formatNumber(tx.block_number)}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-sm font-medium text-gray-500">Value:</span>
          <span class="text-sm text-gray-900">${formatEthValue(tx.value)}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-sm font-medium text-gray-500">Status:</span>
          <span class="text-sm text-gray-900">${getStatusBadge(tx.status)}</span>
        </div>
      </div>
      <div class="mt-4">
        <a href="/transaction-detail.html?hash=${tx.hash}" class="w-full flex justify-center py-2 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500">
          View Full Details
        </a>
      </div>
    </div>
  `;
}

// Generate account result HTML
function generateAccountHTML(account) {
  return `
    <div class="bg-white shadow rounded-lg p-6">
      <h3 class="text-lg font-medium text-gray-900 mb-4">Account Found</h3>
      <div class="space-y-3">
        <div class="flex justify-between">
          <span class="text-sm font-medium text-gray-500">Address:</span>
          <span class="text-sm text-gray-900 font-mono">${truncateAddress(account.address, 8)}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-sm font-medium text-gray-500">Balance:</span>
          <span class="text-sm text-gray-900">${formatEthValue(account.balance)}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-sm font-medium text-gray-500">Transactions:</span>
          <span class="text-sm text-gray-900">${formatNumber(account.transaction_count || 0)}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-sm font-medium text-gray-500">Type:</span>
          <span class="text-sm text-gray-900">${getAccountTypeBadge(account.account_type || 'unknown')}</span>
        </div>
      </div>
      <div class="mt-4">
        <a href="/account-detail.html?address=${account.address}" class="w-full flex justify-center py-2 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500">
          View Full Details
        </a>
      </div>
    </div>
  `;
}

// Generate block result HTML
function generateBlockHTML(block) {
  return `
    <div class="bg-white shadow rounded-lg p-6">
      <h3 class="text-lg font-medium text-gray-900 mb-4">Block Found</h3>
      <div class="space-y-3">
        <div class="flex justify-between">
          <span class="text-sm font-medium text-gray-500">Number:</span>
          <span class="text-sm text-gray-900">${formatNumber(block.number)}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-sm font-medium text-gray-500">Timestamp:</span>
          <span class="text-sm text-gray-900">${formatTimestamp(block.timestamp)}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-sm font-medium text-gray-500">Transactions:</span>
          <span class="text-sm text-gray-900">${formatNumber(block.transaction_count || 0)}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-sm font-medium text-gray-500">Gas Used:</span>
          <span class="text-sm text-gray-900">${formatNumber(block.gas_used || 0)}</span>
        </div>
      </div>
      <div class="mt-4">
        <a href="/block-detail.html?number=${block.number}" class="w-full flex justify-center py-2 px-4 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500">
          View Full Details
        </a>
      </div>
    </div>
  `;
}

// Show/hide states
function showLoading() {
  document.getElementById("loading-state").classList.remove("hidden");
}

function hideLoading() {
  document.getElementById("loading-state").classList.add("hidden");
}

function showError(message = "Unable to perform search. Please try again.") {
  document.getElementById("error-message").textContent = message;
  document.getElementById("error-state").classList.remove("hidden");
}

function showNoResults() {
  document.getElementById("no-results-state").classList.remove("hidden");
}

function hideAllStates() {
  document.getElementById("search-results").classList.add("hidden");
  document.getElementById("error-state").classList.add("hidden");
  document.getElementById("no-results-state").classList.add("hidden");
}

// Handle search from navigation bar
function handleSearchKeyPress(event) {
  if (event.key === "Enter") {
    const query = event.target.value.trim();
    if (query) {
      window.location.href = `/search.html?q=${encodeURIComponent(query)}`;
    }
  }
}

// Handle search from main search input
function handleMainSearchKeyPress(event) {
  if (event.key === "Enter") {
    const query = event.target.value.trim();
    if (query) {
      performSearch(query);
      updateSearchQuery(query);
    }
  }
}

// Update search query display
function updateSearchQuery(query) {
  document.getElementById("search-query-display").textContent = `Searching for: "${query}"`;
}

// Retry search
function retrySearch() {
  const query = document.getElementById("main-search-input").value.trim();
  if (query) {
    performSearch(query);
  }
}

// Initialize page
document.addEventListener("DOMContentLoaded", function() {
  // Get query from URL parameters
  const urlParams = new URLSearchParams(window.location.search);
  const query = urlParams.get('q');
  
  if (query) {
    document.getElementById("main-search-input").value = query;
    document.getElementById("search-input").value = query;
    performSearch(query);
    updateSearchQuery(query);
  }
  
  // Focus on main search input
  document.getElementById("main-search-input").focus();
});
