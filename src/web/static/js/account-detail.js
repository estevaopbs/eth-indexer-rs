// Account Detail Page JavaScript

const API_BASE = "/api";
let accountAddress = null;
let currentPage = 1;
let perPage = 25;
let currentFilter = "all";

// Get account address from URL
function getAccountAddressFromUrl() {
  const urlParams = new URLSearchParams(window.location.search);
  return urlParams.get('address') || urlParams.get('account');
}

// Format number with commas
function formatNumber(num) {
  if (num === null || num === undefined) return "N/A";
  return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

// Format timestamp to readable date
function formatTimestamp(timestamp) {
  if (!timestamp) return "N/A";
  return new Date(timestamp * 1000).toLocaleDateString();
}

// Format ETH value
function formatEthValue(value) {
  if (!value || value === "0") return "0 ETH";
  
  // Convert from Wei to ETH
  const eth = parseFloat(value) / Math.pow(10, 18);
  
  if (eth < 0.001) {
    return "<0.001 ETH";
  } else if (eth < 1) {
    return eth.toFixed(6) + " ETH";
  } else if (eth < 1000) {
    return eth.toFixed(4) + " ETH";
  } else {
    return formatNumber(Math.floor(eth)) + " ETH";
  }
}

// Truncate address
function truncateAddress(address, length = 6) {
  if (!address) return "N/A";
  return `${address.substring(0, length)}...${address.substring(address.length - length)}`;
}

// Truncate hash
function truncateHash(hash, length = 8) {
  if (!hash) return "N/A";
  return `${hash.substring(0, length)}...${hash.substring(hash.length - length)}`;
}

// Get account type badge
function getAccountTypeBadge(type) {
  const typeMap = {
    'eoa': { label: 'EOA', class: 'bg-blue-100 text-blue-800', description: 'Externally Owned Account' },
    'contract': { label: 'Contract', class: 'bg-purple-100 text-purple-800', description: 'Smart Contract' },
    'unknown': { label: 'Unknown', class: 'bg-gray-100 text-gray-800', description: 'Unknown Type' }
  };
  
  const accountType = typeMap[type] || typeMap['unknown'];
  
  return `<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${accountType.class}" title="${accountType.description}">
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

// Get direction badge
function getDirectionBadge(direction, fromAddress, toAddress, accountAddr) {
  const addr = accountAddr.toLowerCase();
  const from = fromAddress ? fromAddress.toLowerCase() : '';
  const to = toAddress ? toAddress.toLowerCase() : '';
  
  if (from === addr && to === addr) {
    return '<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-800">Self</span>';
  } else if (from === addr) {
    return '<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-800">Out</span>';
  } else if (to === addr) {
    return '<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">In</span>';
  } else {
    return '<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-800">Related</span>';
  }
}

// Load account details
async function loadAccountDetails(address) {
  showLoading();
  hideError();
  hideContent();
  
  try {
    const response = await fetch(`${API_BASE}/accounts/${address}`);
    
    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }
    
    const data = await response.json();
    displayAccountDetails(data.account);
    loadAccountTransactions(address);
    
  } catch (error) {
    console.error("Error loading account details:", error);
    showError();
  } finally {
    hideLoading();
  }
}

// Display account details
function displayAccountDetails(account) {
  if (!account) return;
  
  // Update page title and breadcrumb
  document.getElementById("breadcrumb-account").textContent = truncateAddress(account.address, 12);
  document.title = `Account ${truncateAddress(account.address)} - ETH Indexer RS`;
  
  // Update summary cards
  document.getElementById("summary-balance").textContent = formatEthValue(account.balance);
  document.getElementById("summary-transactions").textContent = formatNumber(account.transaction_count || 0);
  document.getElementById("summary-type").innerHTML = getAccountTypeBadge(account.account_type || 'unknown');
  document.getElementById("summary-first-seen").textContent = account.first_seen_block ? 
    `Block ${formatNumber(account.first_seen_block)}` : "Unknown";
  
  // Create detailed information table
  const detailsTable = document.getElementById("account-details-table");
  detailsTable.innerHTML = "";
  
  const details = [
    { label: "Address", value: account.address, copyable: true },
    { label: "Balance", value: `${formatEthValue(account.balance)} (${formatNumber(account.balance)} wei)` },
    { label: "Transaction Count", value: formatNumber(account.transaction_count || 0) },
    { label: "Account Type", value: getAccountTypeBadge(account.account_type || 'unknown') },
    { label: "First Seen Block", value: account.first_seen_block ? 
      `<a href="/block-detail.html?number=${account.first_seen_block}" class="text-blue-600 hover:text-blue-900">${formatNumber(account.first_seen_block)}</a>` : 
      "Unknown" },
    { label: "Last Seen Block", value: account.last_seen_block ? 
      `<a href="/block-detail.html?number=${account.last_seen_block}" class="text-blue-600 hover:text-blue-900">${formatNumber(account.last_seen_block)}</a>` : 
      "Unknown" },
  ];
  
  // Add note if account is not fully indexed
  if (account.note) {
    details.push({ label: "Note", value: account.note });
  }
  
  details.forEach(detail => {
    const row = document.createElement("tr");
    row.className = "hover:bg-gray-50";
    
    let valueContent = detail.value;
    
    if (detail.copyable) {
      valueContent = `<span class="font-mono cursor-pointer hover:bg-gray-100 p-1 rounded break-all" onclick="copyToClipboard('${detail.value}')" title="Click to copy">${detail.value}</span>`;
    }
    
    row.innerHTML = `
      <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-500" style="width: 200px;">
        ${detail.label}
      </td>
      <td class="px-6 py-4 text-sm text-gray-900">
        ${valueContent}
      </td>
    `;
    detailsTable.appendChild(row);
  });
  
  showContent();
}

// Load transactions for this account
async function loadAccountTransactions(address, page = 1, per_page = 25, filter = "all") {
  const transactionsLoading = document.getElementById("transactions-loading");
  const transactionsContent = document.getElementById("transactions-content");
  
  transactionsLoading.classList.remove("hidden");
  transactionsContent.classList.add("hidden");
  
  try {
    // Build query parameters
    let url = `${API_BASE}/transactions?per_page=${per_page}&page=${page}`;
    
    // Add filter based on direction
    if (filter === "sent") {
      url += `&from=${address}`;
    } else if (filter === "received") {
      url += `&to=${address}`;
    } else {
      // For "all", we need to get transactions where this address is either from or to
      // This might require a more sophisticated API endpoint
      url += `&address=${address}`;
    }
    
    const response = await fetch(url);
    
    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }
    
    const data = await response.json();
    displayAccountTransactions(data.transactions || [], address);
    updateTransactionsPagination(page, data.has_next || false);
    document.getElementById("transactions-count").textContent = 
      `${data.transactions ? data.transactions.length : 0} transactions (page ${page})`;
    
  } catch (error) {
    console.error("Error loading account transactions:", error);
    document.getElementById("transactions-count").textContent = "Error loading transactions";
  } finally {
    transactionsLoading.classList.add("hidden");
    transactionsContent.classList.remove("hidden");
  }
}

// Display transactions for account
function displayAccountTransactions(transactions, accountAddr) {
  const tableBody = document.getElementById("account-transactions");
  tableBody.innerHTML = "";
  
  if (transactions.length === 0) {
    tableBody.innerHTML = `
      <tr>
        <td colspan="6" class="px-6 py-4 text-center text-gray-500">
          No transactions found for this account
        </td>
      </tr>
    `;
    return;
  }
  
  transactions.forEach(tx => {
    const addr = accountAddr.toLowerCase();
    const from = tx.from_address ? tx.from_address.toLowerCase() : '';
    const to = tx.to_address ? tx.to_address.toLowerCase() : '';
    
    // Determine counterpart address
    let counterpart = "N/A";
    if (from === addr && to && to !== addr) {
      counterpart = tx.to_address;
    } else if (to === addr && from !== addr) {
      counterpart = tx.from_address;
    } else if (from === addr && !to) {
      counterpart = "Contract Creation";
    }
    
    const row = document.createElement("tr");
    row.className = "hover:bg-gray-50";
    row.innerHTML = `
      <td class="px-6 py-4 whitespace-nowrap">
        <a href="/transaction-detail.html?hash=${tx.hash}" class="text-blue-600 hover:text-blue-900 font-mono text-sm">
          ${truncateHash(tx.hash)}
        </a>
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        <a href="/block-detail.html?number=${tx.block_number}" class="text-blue-600 hover:text-blue-900 text-sm">
          ${formatNumber(tx.block_number)}
        </a>
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        ${getDirectionBadge(null, tx.from_address, tx.to_address, accountAddr)}
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        ${counterpart === "Contract Creation" || counterpart === "N/A" ? 
          `<span class="text-gray-400">${counterpart}</span>` :
          `<a href="/account-detail.html?address=${counterpart}" class="text-blue-600 hover:text-blue-900 font-mono text-sm">${truncateAddress(counterpart)}</a>`
        }
      </td>
      <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
        ${formatEthValue(tx.value)}
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        ${getStatusBadge(tx.status)}
      </td>
    `;
    tableBody.appendChild(row);
  });
}

// Update transactions pagination
function updateTransactionsPagination(page, hasNext) {
  currentPage = page;
  
  document.getElementById("page-info").textContent = `Page ${page}`;
  document.getElementById("prev-page").disabled = page <= 1;
  document.getElementById("next-page").disabled = !hasNext;
}

// Copy to clipboard function
function copyToClipboard(text) {
  navigator.clipboard.writeText(text).then(() => {
    // Show a simple notification
    const notification = document.createElement('div');
    notification.className = 'fixed top-4 right-4 bg-green-500 text-white px-4 py-2 rounded shadow-lg z-50';
    notification.textContent = 'Copied to clipboard!';
    document.body.appendChild(notification);
    
    setTimeout(() => {
      document.body.removeChild(notification);
    }, 2000);
  }).catch(err => {
    console.error('Could not copy text: ', err);
  });
}

// Show/hide states
function showLoading() {
  document.getElementById("loading-state").classList.remove("hidden");
}

function hideLoading() {
  document.getElementById("loading-state").classList.add("hidden");
}

function showError() {
  document.getElementById("error-state").classList.remove("hidden");
}

function hideError() {
  document.getElementById("error-state").classList.add("hidden");
}

function showContent() {
  document.getElementById("account-content").classList.remove("hidden");
}

function hideContent() {
  document.getElementById("account-content").classList.add("hidden");
}

// Initialize page
document.addEventListener("DOMContentLoaded", function() {
  accountAddress = getAccountAddressFromUrl();
  
  if (!accountAddress) {
    showError();
    document.querySelector("#error-state h3").textContent = "No account specified";
    document.querySelector("#error-state p").textContent = "Please provide an account address in the URL.";
    hideLoading();
    return;
  }
  
  loadAccountDetails(accountAddress);
  
  // Setup event listeners
  document.getElementById("retry-btn").addEventListener("click", function() {
    if (accountAddress) {
      loadAccountDetails(accountAddress);
    }
  });
  
  document.getElementById("tx-filter").addEventListener("change", function() {
    currentFilter = this.value;
    currentPage = 1;
    loadAccountTransactions(accountAddress, currentPage, perPage, currentFilter);
  });
  
  document.getElementById("per-page-select").addEventListener("change", function() {
    perPage = parseInt(this.value);
    currentPage = 1;
    loadAccountTransactions(accountAddress, currentPage, perPage, currentFilter);
  });
  
  document.getElementById("prev-page").addEventListener("click", function() {
    if (currentPage > 1) {
      loadAccountTransactions(accountAddress, currentPage - 1, perPage, currentFilter);
    }
  });
  
  document.getElementById("next-page").addEventListener("click", function() {
    loadAccountTransactions(accountAddress, currentPage + 1, perPage, currentFilter);
  });
});
