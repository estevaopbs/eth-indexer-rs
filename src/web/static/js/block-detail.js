// Block Detail Page JavaScript

const API_BASE = "/api";
let blockNumber = null;

// Get block number from URL
function getBlockNumberFromUrl() {
  const urlParams = new URLSearchParams(window.location.search);
  return urlParams.get('number') || urlParams.get('block');
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
    return eth.toFixed(4) + " ETH";
  } else {
    return eth.toFixed(2) + " ETH";
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

// Get status badge
function getStatusBadge(status) {
  const isSuccess = status === "success" || status === true || status === 1;
  
  if (isSuccess) {
    return '<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">Success</span>';
  } else {
    return '<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-800">Failed</span>';
  }
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

// Format size in bytes
function formatSize(bytes) {
  if (!bytes) return "0 B";
  const sizes = ['B', 'KB', 'MB', 'GB'];
  if (bytes === 0) return '0 B';
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return Math.round(bytes / Math.pow(1024, i) * 100) / 100 + ' ' + sizes[i];
}

// Load block details
async function loadBlockDetails(blockNum) {
  showLoading();
  hideError();
  hideContent();
  
  try {
    const response = await fetch(`${API_BASE}/blocks/${blockNum}`);
    
    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }
    
    const data = await response.json();
    displayBlockDetails(data.block);
    loadBlockTransactions(blockNum);
    
  } catch (error) {
    console.error("Error loading block details:", error);
    showError();
  } finally {
    hideLoading();
  }
}

// Display block details
function displayBlockDetails(block) {
  if (!block) return;
  
  // Update page title and breadcrumb
  document.getElementById("block-title").textContent = `Block ${formatNumber(block.number)}`;
  document.getElementById("breadcrumb-block").textContent = `Block ${formatNumber(block.number)}`;
  document.title = `Block ${block.number} - ETH Indexer RS`;
  
  // Update summary cards
  document.getElementById("summary-transactions").textContent = formatNumber(block.transaction_count || 0);
  document.getElementById("summary-gas-used").textContent = formatGas(block.gas_used);
  document.getElementById("summary-base-fee").textContent = block.base_fee_per_gas ? 
    `${(parseInt(block.base_fee_per_gas) / Math.pow(10, 9)).toFixed(2)} Gwei` : "N/A";
  document.getElementById("summary-size").textContent = formatSize(block.size_bytes);
  
  // Create detailed information table
  const detailsTable = document.getElementById("block-details-table");
  detailsTable.innerHTML = "";
  
  const details = [
    { label: "Block Number", value: formatNumber(block.number) },
    { label: "Block Hash", value: block.hash, copyable: true },
    { label: "Parent Hash", value: block.parent_hash, copyable: true },
    { label: "Timestamp", value: formatTimestamp(block.timestamp) },
    { label: "Miner", value: formatMiner(block.miner), copyable: block.miner && !isNA(block.miner) },
    { label: "Gas Used", value: `${formatNumber(block.gas_used)} (${((block.gas_used / block.gas_limit) * 100).toFixed(2)}%)` },
    { label: "Gas Limit", value: formatNumber(block.gas_limit) },
    { label: "Gas Utilization", value: `${(block.gas_utilization || 0).toFixed(2)}%` },
    { label: "Base Fee per Gas", value: formatBaseFee(block.base_fee_per_gas) },
    { label: "Burnt Fees", value: formatEthValue(block.burnt_fees) },
    { label: "Priority Fees", value: formatEthValue(block.priority_fees) },
    { label: "Block Reward", value: formatEthValue(block.block_reward) },
    { label: "Transaction Count", value: formatNumber(block.transaction_count || 0) },
    { label: "Block Size", value: formatSize(block.size_bytes) },
    { label: "Status", value: formatStatus(block.status) },
    { label: "State Root", value: block.state_root, copyable: true },
    { label: "Nonce", value: formatNonce(block.nonce) },
    { label: "Extra Data", value: formatExtraData(block.extra_data) },
    { label: "Difficulty", value: block.difficulty || "N/A (Post-Merge)" },
    { label: "Withdrawals Root", value: block.withdrawals_root || "N/A", copyable: !!block.withdrawals_root },
    { label: "Withdrawal Count", value: formatNumber(block.withdrawal_count) || "0" },
    { label: "Blob Gas Used", value: formatBlobGas(block.blob_gas_used) },
    { label: "Excess Blob Gas", value: formatNumber(block.excess_blob_gas) || "0" },
    { label: "Blob Utilization", value: formatBlobUtilization(block.blob_utilization) },
    { label: "Blob Transactions", value: formatNumber(block.blob_transactions) || "0" },
    { label: "Blob Size", value: formatBlobSize(block.blob_size) },
    { label: "Blob Gas Price", value: formatBlobGasPrice(block.blob_gas_price) },
    { label: "Beacon Slot", value: formatNumber(block.slot) || "N/A" },
    { label: "Proposer Index", value: formatNumber(block.proposer_index) || "N/A" },
    { label: "Epoch", value: formatNumber(block.epoch) || "N/A" },
    { label: "Slot Root", value: block.slot_root || "N/A", copyable: !!block.slot_root },
    { label: "Parent Root", value: block.parent_root || "N/A", copyable: !!block.parent_root },
    { label: "Beacon Deposit Count", value: formatNumber(block.beacon_deposit_count) || "N/A" },
    { label: "Graffiti", value: formatGraffiti(block.graffiti) },
    { label: "Randao Reveal", value: block.randao_reveal || "N/A", copyable: !!block.randao_reveal },
    { label: "Randao Mix", value: block.randao_mix || "N/A", copyable: !!block.randao_mix }
  ];
  
  details.forEach(detail => {
    const row = document.createElement("tr");
    row.className = "hover:bg-gray-50";
    row.innerHTML = `
      <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-500" style="width: 200px;">
        ${detail.label}
      </td>
      <td class="px-6 py-4 text-sm text-gray-900 break-all">
        ${detail.copyable ? 
          `<span class="font-mono cursor-pointer hover:bg-gray-100 p-1 rounded" onclick="copyToClipboard('${detail.value}')" title="Click to copy">${detail.value}</span>` :
          detail.value
        }
      </td>
    `;
    detailsTable.appendChild(row);
  });
  
  showContent();
}

// Load transactions for this block
async function loadBlockTransactions(blockNum) {
  const transactionsLoading = document.getElementById("transactions-loading");
  const transactionsContent = document.getElementById("transactions-content");
  
  transactionsLoading.classList.remove("hidden");
  transactionsContent.classList.add("hidden");
  
  try {
    // Use the block-specific transactions endpoint if it exists, otherwise filter by block
    const response = await fetch(`${API_BASE}/transactions?block=${blockNum}&per_page=100`);
    
    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }
    
    const data = await response.json();
    displayBlockTransactions(data.transactions || []);
    document.getElementById("transactions-count").textContent = `${data.transactions ? data.transactions.length : 0} transactions`;
    
  } catch (error) {
    console.error("Error loading block transactions:", error);
    document.getElementById("transactions-count").textContent = "Error loading transactions";
  } finally {
    transactionsLoading.classList.add("hidden");
    transactionsContent.classList.remove("hidden");
  }
}

// Display transactions in block
function displayBlockTransactions(transactions) {
  const tableBody = document.getElementById("block-transactions");
  tableBody.innerHTML = "";
  
  if (transactions.length === 0) {
    tableBody.innerHTML = `
      <tr>
        <td colspan="6" class="px-6 py-4 text-center text-gray-500">
          No transactions found in this block
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
        <a href="/transaction-detail.html?hash=${tx.hash}" class="text-blue-600 hover:text-blue-900 font-mono text-sm">
          ${truncateHash(tx.hash)}
        </a>
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        <a href="/account-detail.html?address=${tx.from_address}" class="text-blue-600 hover:text-blue-900 font-mono text-sm">
          ${truncateAddress(tx.from_address)}
        </a>
      </td>
      <td class="px-6 py-4 whitespace-nowrap">
        ${tx.to_address ? 
          `<a href="/account-detail.html?address=${tx.to_address}" class="text-blue-600 hover:text-blue-900 font-mono text-sm">${truncateAddress(tx.to_address)}</a>` :
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

// Copy to clipboard function
function copyToClipboard(text) {
  navigator.clipboard.writeText(text).then(() => {
    // Show a simple notification (you could enhance this)
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
  document.getElementById("block-content").classList.remove("hidden");
}

function hideContent() {
  document.getElementById("block-content").classList.add("hidden");
}

// Format miner address
function formatMiner(miner) {
  if (!miner) return "N/A";
  // Remove Rust's Some() wrapper if present
  const cleanMiner = miner.replace(/^Some\(/, '').replace(/\)$/, '');
  return cleanMiner;
}

// Check if value is N/A
function isNA(value) {
  return !value || value === "N/A" || value === "null" || value === "None";
}

// Format base fee per gas
function formatBaseFee(baseFee) {
  if (!baseFee) return "N/A";
  const fee = parseInt(baseFee);
  if (isNaN(fee)) return "N/A";
  return `${formatNumber(fee)} wei (${(fee / Math.pow(10, 9)).toFixed(2)} Gwei)`;
}

// Format nonce
function formatNonce(nonce) {
  if (!nonce || nonce === "0x0000000000000000") return "0";
  return nonce;
}

// Format extra data
function formatExtraData(extraData) {
  if (!extraData) return "N/A";
  
  // Remove Bytes() wrapper if present
  let cleaned = extraData.replace(/^Bytes\(/, '').replace(/\)$/, '');
  
  // Try to decode hex to ASCII if it looks like hex
  if (cleaned.startsWith('0x')) {
    try {
      const hex = cleaned.slice(2);
      let ascii = '';
      for (let i = 0; i < hex.length; i += 2) {
        const byte = parseInt(hex.substr(i, 2), 16);
        if (byte >= 32 && byte <= 126) { // Printable ASCII
          ascii += String.fromCharCode(byte);
        } else {
          ascii += '.';
        }
      }
      if (ascii.length > 0) {
        return `${cleaned} ("${ascii}")`;
      }
    } catch (e) {
      // If decoding fails, just return the hex
    }
  }
  
  return cleaned;
}

// Format status
function formatStatus(status) {
  if (!status) return "unknown";
  
  const statusColors = {
    'finalized': 'bg-green-100 text-green-800',
    'safe': 'bg-blue-100 text-blue-800',
    'pending': 'bg-yellow-100 text-yellow-800',
    'latest': 'bg-purple-100 text-purple-800'
  };
  
  const colorClass = statusColors[status] || 'bg-gray-100 text-gray-800';
  
  return `<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${colorClass}">
    ${status.charAt(0).toUpperCase() + status.slice(1)}
  </span>`;
}

// Format blob gas
function formatBlobGas(blobGas) {
  if (!blobGas || blobGas === 0) return "0";
  return formatNumber(blobGas);
}

// Format blob utilization
function formatBlobUtilization(utilization) {
  if (utilization === null || utilization === undefined) return "N/A";
  return `${utilization.toFixed(2)}%`;
}

// Format blob size
function formatBlobSize(blobSize) {
  if (!blobSize || blobSize === 0) return "0 KB";
  return formatSize(blobSize * 1024); // Convert from KB to bytes for formatSize
}

// Format blob gas price
function formatBlobGasPrice(price) {
  if (!price || price === "0") return "1 wei";
  const priceNum = parseInt(price);
  if (isNaN(priceNum)) return "N/A";
  return `${formatNumber(priceNum)} wei`;
}

// Format graffiti
function formatGraffiti(graffiti) {
  if (!graffiti) return "N/A";
  
  // Try to decode hex to ASCII if it looks like hex
  if (graffiti.startsWith('0x')) {
    try {
      const hex = graffiti.slice(2);
      let ascii = '';
      for (let i = 0; i < hex.length; i += 2) {
        const byte = parseInt(hex.substr(i, 2), 16);
        if (byte >= 32 && byte <= 126) { // Printable ASCII
          ascii += String.fromCharCode(byte);
        } else if (byte !== 0) { // Skip null bytes
          ascii += '.';
        }
      }
      if (ascii.trim().length > 0) {
        return `"${ascii.trim()}"`;
      }
    } catch (e) {
      // If decoding fails, just return the hex
    }
  }
  
  return graffiti;
}

// Initialize page
document.addEventListener("DOMContentLoaded", function() {
  blockNumber = getBlockNumberFromUrl();
  
  if (!blockNumber) {
    showError();
    document.querySelector("#error-state h3").textContent = "No block specified";
    document.querySelector("#error-state p").textContent = "Please provide a block number in the URL.";
    hideLoading();
    return;
  }
  
  loadBlockDetails(blockNumber);
  
  // Setup retry button
  document.getElementById("retry-btn").addEventListener("click", function() {
    if (blockNumber) {
      loadBlockDetails(blockNumber);
    }
  });
});
