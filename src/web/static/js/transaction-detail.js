// Transaction Detail Page JavaScript

const API_BASE = "/api";
let transactionHash = null;

// Get transaction hash from URL
function getTransactionHashFromUrl() {
  const urlParams = new URLSearchParams(window.location.search);
  return urlParams.get('hash') || urlParams.get('tx');
}

// Format number with commas
function formatNumber(num) {
  if (num === null || num === undefined) return "N/A";
  return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

// Format timestamp to readable date
function formatTimestamp(timestamp) {
  if (!timestamp) return "N/A";
  return new Date(timestamp * 1000).toLocaleString();
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
  } else {
    return eth.toFixed(4) + " ETH";
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

// Format gas amount
function formatGas(gas) {
  if (!gas) return "0";
  const gasNum = parseInt(gas);
  if (gasNum > 1000000) {
    return (gasNum / 1000000).toFixed(2) + "M";
  } else if (gasNum > 1000) {
    return (gasNum / 1000).toFixed(1) + "K";
  }
  return formatNumber(gasNum);
}

// Format gas price to Gwei
function formatGasPrice(gasPrice) {
  if (!gasPrice) return "0 Gwei";
  const gwei = parseInt(gasPrice) / Math.pow(10, 9);
  return gwei.toFixed(2) + " Gwei";
}

// Load transaction details
async function loadTransactionDetails(txHash) {
  showLoading();
  hideError();
  hideContent();
  
  try {
    const response = await fetch(`${API_BASE}/transactions/${txHash}`);
    
    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }
    
    const data = await response.json();
    displayTransactionDetails(data.transaction, data.logs || []);
    loadTokenTransfers(txHash);
    
  } catch (error) {
    console.error("Error loading transaction details:", error);
    showError();
  } finally {
    hideLoading();
  }
}

// Display transaction details
function displayTransactionDetails(tx, logs) {
  if (!tx) return;
  
  // Update page title and breadcrumb
  document.getElementById("breadcrumb-tx").textContent = truncateHash(tx.hash, 12);
  document.title = `Transaction ${truncateHash(tx.hash)} - ETH Indexer RS`;
  
  // Update status banner
  updateStatusBanner(tx);
  
  // Update summary cards
  document.getElementById("summary-value").textContent = formatEthValue(tx.value);
  document.getElementById("summary-gas-used").textContent = formatGas(tx.gas_used);
  document.getElementById("summary-gas-price").textContent = formatGasPrice(tx.gas_price);
  document.getElementById("summary-block").innerHTML = 
    `<a href="/block-detail.html?number=${tx.block_number}" class="text-blue-600 hover:text-blue-900">${formatNumber(tx.block_number)}</a>`;
  
  // Create detailed information table
  const detailsTable = document.getElementById("transaction-details-table");
  detailsTable.innerHTML = "";
  
  const details = [
    { label: "Transaction Hash", value: tx.hash, copyable: true },
    { label: "Status", value: getStatusText(tx.status) },
    { label: "Block Number", value: tx.block_number, linkable: true, linkType: "block" },
    { label: "Transaction Index", value: tx.transaction_index || "N/A" },
    { label: "From", value: tx.from_address, copyable: true, linkable: true, linkType: "account" },
    { label: "To", value: tx.to_address || "Contract Creation", copyable: tx.to_address, linkable: tx.to_address, linkType: "account" },
    { label: "Value", value: `${formatEthValue(tx.value)} (${formatNumber(tx.value)} wei)` },
    { label: "Gas Limit", value: formatNumber(tx.gas_limit) },
    { label: "Gas Used", value: `${formatNumber(tx.gas_used)} (${tx.gas_limit ? ((tx.gas_used / tx.gas_limit) * 100).toFixed(2) : 0}%)` },
    { label: "Gas Price", value: `${formatGasPrice(tx.gas_price)} (${formatNumber(tx.gas_price)} wei)` },
    { label: "Transaction Fee", value: tx.gas_used && tx.gas_price ? formatEthValue((tx.gas_used * tx.gas_price).toString()) : "N/A" },
    { label: "Nonce", value: tx.nonce || "N/A" },
    { label: "Input Data", value: tx.input_data || "0x", copyable: true, expandable: true }
  ];
  
  details.forEach(detail => {
    const row = document.createElement("tr");
    row.className = "hover:bg-gray-50";
    
    let valueContent = detail.value;
    
    if (detail.linkable && detail.linkType === "block") {
      valueContent = `<a href="/block-detail.html?number=${detail.value}" class="text-blue-600 hover:text-blue-900">${formatNumber(detail.value)}</a>`;
    } else if (detail.linkable && detail.linkType === "account" && detail.value !== "Contract Creation") {
      valueContent = `<a href="/account-detail.html?address=${detail.value}" class="text-blue-600 hover:text-blue-900 font-mono">${detail.value}</a>`;
    } else if (detail.copyable) {
      valueContent = `<span class="font-mono cursor-pointer hover:bg-gray-100 p-1 rounded break-all" onclick="copyToClipboard('${detail.copyable === true ? detail.value : detail.copyable}')" title="Click to copy">${detail.value}</span>`;
    }
    
    if (detail.expandable && detail.value && detail.value.length > 100) {
      const shortValue = detail.value.substring(0, 100) + "...";
      valueContent = `
        <div>
          <span class="font-mono break-all" id="input-short">${shortValue}</span>
          <span class="font-mono break-all hidden" id="input-full">${detail.value}</span>
          <button onclick="toggleInputData()" class="ml-2 text-blue-600 hover:text-blue-800 text-sm">Show More</button>
        </div>
      `;
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
  
  // Display logs if any
  if (logs && logs.length > 0) {
    displayTransactionLogs(logs);
  }
  
  showContent();
}

// Update status banner
function updateStatusBanner(tx) {
  const banner = document.getElementById("status-banner");
  const icon = document.getElementById("status-icon");
  const title = document.getElementById("status-title");
  const description = document.getElementById("status-description");
  
  const isSuccess = tx.status === "success" || tx.status === true || tx.status === 1;
  
  if (isSuccess) {
    banner.className = "mb-6 p-4 rounded-md bg-green-50 border border-green-200";
    icon.className = "h-5 w-5 text-green-400";
    icon.innerHTML = '<path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />';
    title.className = "text-sm font-medium text-green-800";
    title.textContent = "Transaction Successful";
    description.className = "mt-2 text-sm text-green-700";
    description.textContent = "This transaction was successfully processed and included in the blockchain.";
  } else {
    banner.className = "mb-6 p-4 rounded-md bg-red-50 border border-red-200";
    icon.className = "h-5 w-5 text-red-400";
    icon.innerHTML = '<path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />';
    title.className = "text-sm font-medium text-red-800";
    title.textContent = "Transaction Failed";
    description.className = "mt-2 text-sm text-red-700";
    description.textContent = "This transaction failed during execution and was reverted.";
  }
}

// Get status text
function getStatusText(status) {
  const isSuccess = status === "success" || status === true || status === 1;
  return isSuccess ? 
    '<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">Success</span>' :
    '<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-800">Failed</span>';
}

// Display transaction logs
function displayTransactionLogs(logs) {
  const logsSection = document.getElementById("logs-section");
  const logsTable = document.getElementById("transaction-logs");
  const logsCount = document.getElementById("logs-count");
  
  logsCount.textContent = `${logs.length} logs`;
  logsTable.innerHTML = "";
  
  if (logs.length === 0) {
    logsTable.innerHTML = `
      <tr>
        <td colspan="3" class="px-6 py-4 text-center text-gray-500">
          No logs found for this transaction
        </td>
      </tr>
    `;
  } else {
    logs.forEach((log, index) => {
      const row = document.createElement("tr");
      row.className = "hover:bg-gray-50";
      row.innerHTML = `
        <td class="px-6 py-4 whitespace-nowrap">
          <a href="/account-detail.html?address=${log.address}" class="text-blue-600 hover:text-blue-900 font-mono text-sm">
            ${truncateAddress(log.address)}
          </a>
        </td>
        <td class="px-6 py-4">
          <div class="text-sm text-gray-900 font-mono break-all">
            ${log.topics ? log.topics.slice(0, 2).map(topic => truncateHash(topic, 16)).join('<br>') : 'No topics'}
            ${log.topics && log.topics.length > 2 ? '<br>...' : ''}
          </div>
        </td>
        <td class="px-6 py-4">
          <div class="text-sm text-gray-900 font-mono break-all">
            ${log.data ? truncateHash(log.data, 32) : 'No data'}
          </div>
        </td>
      `;
      logsTable.appendChild(row);
    });
  }
  
  logsSection.classList.remove("hidden");
}

// Toggle input data display
function toggleInputData() {
  const shortElement = document.getElementById("input-short");
  const fullElement = document.getElementById("input-full");
  const button = event.target;
  
  if (shortElement.classList.contains("hidden")) {
    shortElement.classList.remove("hidden");
    fullElement.classList.add("hidden");
    button.textContent = "Show More";
  } else {
    shortElement.classList.add("hidden");
    fullElement.classList.remove("hidden");
    button.textContent = "Show Less";
  }
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
  document.getElementById("transaction-content").classList.remove("hidden");
}

function hideContent() {
  document.getElementById("transaction-content").classList.add("hidden");
}

// Load token transfers for the transaction
async function loadTokenTransfers(txHash) {
  showTokenTransfersLoading();
  
  try {
    const response = await fetch(`${API_BASE}/transactions/${txHash}/token-transfers`);
    
    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }
    
    const data = await response.json();
    displayTokenTransfers(data.token_transfers || []);
    
  } catch (error) {
    console.error("Error loading token transfers:", error);
    showNoTokenTransfers();
  }
}

// Display token transfers
function displayTokenTransfers(transfers) {
  const tableBody = document.getElementById("token-transfers-table");
  
  if (!transfers || transfers.length === 0) {
    showNoTokenTransfers();
    return;
  }
  
  tableBody.innerHTML = "";
  
  transfers.forEach(transfer => {
    const row = document.createElement("tr");
    row.className = "hover:bg-gray-50";
    
    // Format token info
    const tokenName = transfer.token?.name || "Unknown Token";
    const tokenSymbol = transfer.token?.symbol || "???";
    const decimals = transfer.token?.decimals || 18;
    
    // Format amount
    const formattedAmount = formatTokenAmount(transfer.amount, decimals);
    
    row.innerHTML = `
      <td class="px-6 py-4 whitespace-nowrap">
        <div class="text-sm font-medium text-gray-900">${tokenName}</div>
        <div class="text-sm text-gray-500">${tokenSymbol}</div>
        <div class="text-xs text-gray-400 font-mono">
          <a href="/account-detail.html?address=${transfer.token_address}" 
             class="text-blue-600 hover:text-blue-900">
            ${truncateAddress(transfer.token_address)}
          </a>
        </div>
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        <a href="/account-detail.html?address=${transfer.from_address}" 
           class="text-sm text-blue-600 hover:text-blue-900 font-mono">
          ${truncateAddress(transfer.from_address)}
        </a>
        <button onclick="copyToClipboard('${transfer.from_address}')" 
                class="ml-2 text-gray-400 hover:text-gray-600">
          <svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                  d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
          </svg>
        </button>
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        <a href="/account-detail.html?address=${transfer.to_address}" 
           class="text-sm text-blue-600 hover:text-blue-900 font-mono">
          ${truncateAddress(transfer.to_address)}
        </a>
        <button onclick="copyToClipboard('${transfer.to_address}')" 
                class="ml-2 text-gray-400 hover:text-gray-600">
          <svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                  d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
          </svg>
        </button>
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        <span class="text-sm font-medium text-gray-900">${formattedAmount} ${tokenSymbol}</span>
      </td>
    `;
    
    tableBody.appendChild(row);
  });
  
  hideTokenTransfersLoading();
  showTokenTransfersContent();
}

// Format token amount based on decimals
function formatTokenAmount(amount, decimals) {
  if (!amount || amount === "0") return "0";
  
  const dec = decimals || 18;
  const divisor = Math.pow(10, dec);
  const formatted = parseFloat(amount) / divisor;
  
  if (formatted < 0.0001) {
    return "<0.0001";
  } else if (formatted < 1) {
    return formatted.toFixed(6);
  } else if (formatted < 1000) {
    return formatted.toFixed(4);
  } else {
    return formatNumber(Math.floor(formatted));
  }
}

// Token transfers UI state functions
function showTokenTransfersLoading() {
  document.getElementById("token-transfers-loading").classList.remove("hidden");
  document.getElementById("token-transfers-content").classList.add("hidden");
  document.getElementById("no-token-transfers").classList.add("hidden");
}

function hideTokenTransfersLoading() {
  document.getElementById("token-transfers-loading").classList.add("hidden");
}

function showTokenTransfersContent() {
  document.getElementById("token-transfers-content").classList.remove("hidden");
  document.getElementById("no-token-transfers").classList.add("hidden");
}

function showNoTokenTransfers() {
  hideTokenTransfersLoading();
  document.getElementById("token-transfers-content").classList.add("hidden");
  document.getElementById("no-token-transfers").classList.remove("hidden");
}

// Copy to clipboard function (if not already exists)
function copyToClipboard(text) {
  navigator.clipboard.writeText(text).then(function() {
    // Could add a toast notification here
    console.log('Copied to clipboard:', text);
  }).catch(function(err) {
    console.error('Could not copy text: ', err);
  });
}

// Initialize page
document.addEventListener("DOMContentLoaded", function() {
  transactionHash = getTransactionHashFromUrl();
  
  if (!transactionHash) {
    showError();
    document.querySelector("#error-state h3").textContent = "No transaction specified";
    document.querySelector("#error-state p").textContent = "Please provide a transaction hash in the URL.";
    hideLoading();
    return;
  }
  
  loadTransactionDetails(transactionHash);
  
  // Setup retry button
  document.getElementById("retry-btn").addEventListener("click", function() {
    if (transactionHash) {
      loadTransactionDetails(transactionHash);
    }
  });
});
