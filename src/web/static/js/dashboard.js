// ETH Indexer RS - Dashboard JavaScript

// API Base URL
const API_BASE = "/api";
let gasChart = null; // Global chart instances
let txsChart = null;
let latestNetworkBlock = 0; // Track latest network block for progress calculation

// Format number with commas
function formatNumber(num) {
  return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

// Format timestamp to readable date
function formatTimestamp(timestamp) {
  return new Date(timestamp * 1000).toLocaleString();
}

// Truncate address/hash
function truncateHash(hash, length = 10) {
  if (!hash) return "";
  return `${hash.substring(0, length)}...${hash.substring(
    hash.length - length
  )}`;
}

// Format ETH value
function formatEth(value) {
  if (!value) return "0";
  // Convert from wei to ETH
  const ethValue = Number(BigInt(value)) / Number(10 ** 18);

  // Format to 5 decimal places and remove trailing zeros
  const formatted = ethValue.toFixed(5).replace(/\.?0+$/, '');

  return formatted;
}

// Load all data without loading indicators (unified 1-second refresh)
async function refreshAllData() {
  try {
    await Promise.all([
      loadStats(),
      loadRecentBlocksDelta(),
      loadRecentTransactionsDelta()
    ]);
  } catch (error) {
    console.error("Error refreshing data:", error);
    // Don't show empty tables on error - keep existing content
  }
}

// Initialize data on first load
async function initializeData() {
  try {
    // Load initial stats
    await loadStats();

    // Load initial blocks and set up delta tracking
    const blocksResponse = await fetch(`${API_BASE}/blocks?per_page=5`);
    if (blocksResponse.ok) {
      const blocksData = await blocksResponse.json();
      if (blocksData.blocks && blocksData.blocks.length > 0) {
        // Set up blocks table
        const blocksList = document.getElementById("recent-blocks");
        const fragment = document.createDocumentFragment();

        for (const block of blocksData.blocks) {
          const row = document.createElement("tr");
          row.className = "hover:bg-gray-50";
          row.innerHTML = `
            <td class="px-3 py-4 whitespace-nowrap text-left">
              <a href="/blocks.html?number=${block.number}" class="text-blue-600 hover:text-blue-900">${block.number}</a>
            </td>
            <td class="px-3 py-4 whitespace-nowrap text-left">${block.transaction_count}</td>
            <td class="px-3 py-4 whitespace-nowrap text-left">${formatNumber(block.gas_used || 0)}</td>
            <td class="px-3 py-4 whitespace-nowrap text-right">${formatTimestamp(block.timestamp)}</td>
          `;
          fragment.appendChild(row);
        }

        blocksList.innerHTML = "";
        blocksList.appendChild(fragment);

        // Initialize lastBlockNumber for delta updates
        lastBlockNumber = Math.max(...blocksData.blocks.map(b => b.number));

        // Initialize charts
        // blocksData.blocks comes in descending order (newest first)
        // Charts need chronological order (oldest first), so reverse
        createGasChart(blocksData.blocks.slice().reverse());
        createTxsChart(blocksData.blocks.slice().reverse());
      } else {
        // No blocks data available, initialize empty charts
        showEmptyCharts();
      }
    } else {
      // API call failed, initialize empty charts
      showEmptyCharts();
    }

    // Load initial transactions and set up delta tracking
    const txsResponse = await fetch(`${API_BASE}/transactions?per_page=5`);
    if (txsResponse.ok) {
      const txsData = await txsResponse.json();
      if (txsData.transactions && txsData.transactions.length > 0) {
        // Set up transactions table
        const txsList = document.getElementById("recent-txs");
        const fragment = document.createDocumentFragment();

        for (const tx of txsData.transactions) {
          const row = document.createElement("tr");
          row.className = "hover:bg-gray-50";
          row.innerHTML = `
            <td class="px-3 py-4 whitespace-nowrap text-sm font-mono truncate">
              <a href="/transactions.html?hash=${tx.hash}" class="hash-link">${truncateHash(tx.hash)}</a>
            </td>
            <td class="px-3 py-4 whitespace-nowrap text-sm font-mono truncate">
              <a href="/accounts.html?address=${tx.from_address}" class="hash-link">${truncateHash(tx.from_address)}</a>
            </td>
            <td class="px-3 py-4 whitespace-nowrap text-sm font-mono truncate">
              ${tx.to_address
              ? `<a href="/accounts.html?address=${tx.to_address}" class="hash-link">${truncateHash(tx.to_address)}</a>`
              : '<span class="text-gray-500 text-sm">Contract Creation</span>'
            }
            </td>
            <td class="px-3 py-4 whitespace-nowrap text-sm font-mono text-right">${formatEth(tx.value)}</td>
          `;
          fragment.appendChild(row);
        }

        txsList.innerHTML = "";
        txsList.appendChild(fragment);

        // Initialize lastTransactionHash for delta updates
        lastTransactionHash = txsData.transactions[0].hash;
      }
    }

  } catch (error) {
    console.error("Error initializing data:", error);
    showEmptyBlocksTable();
    showEmptyTransactionsTable();
    showEmptyCharts();
  }
}

// Load indexer stats
async function loadStats() {
  try {
    console.log('Loading stats...');
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 10000); // 10 second timeout

    const [statsResponse, networkResponse, networkStatsResponse] = await Promise.all([
      fetch(`${API_BASE}/stats`, { signal: controller.signal }),
      fetch(`${API_BASE}/network/latest`, { signal: controller.signal }),
      fetch(`${API_BASE}/network/stats`, { signal: controller.signal })
    ]);

    clearTimeout(timeoutId);
    console.log('Stats response:', statsResponse.status);

    if (!statsResponse.ok) {
      throw new Error(`Stats API returned ${statsResponse.status}`);
    }

    const data = await statsResponse.json();
    console.log('Stats data:', data);

    let networkData = { latest_network_block: 0 };
    if (networkResponse.ok) {
      networkData = await networkResponse.json();
    }

    let networkStatsData = {
      latest_network_block: 0,
      total_network_transactions: 0,
      total_network_accounts: 0
    };
    if (networkStatsResponse.ok) {
      networkStatsData = await networkStatsResponse.json();
    }

    latestNetworkBlock = networkData.latest_network_block || networkStatsData.latest_network_block;

    // Update network stats
    updateStatWithAnimation("latest-network-block", formatNumber(networkStatsData.latest_network_block));
    updateStatWithAnimation("total-network-txs", formatNumber(networkStatsData.total_network_transactions));
    updateStatWithAnimation("total-network-accounts", formatNumber(networkStatsData.total_network_accounts));

    // Update indexed stats
    updateStatWithAnimation("latest-indexed-block", formatNumber(data.latest_block));
    updateStatWithAnimation("total-indexed-txs", formatNumber(data.real_transactions_indexed));
    updateStatWithAnimation("total-blockchain-txs", formatNumber(data.total_blockchain_transactions));
    updateStatWithAnimation("total-indexed-accounts", formatNumber(data.total_accounts));
    updateStatWithAnimation("sync-status", data.indexer_status);

    // Update progress bar
    updateProgressBar(data.latest_block, latestNetworkBlock, data.start_block || 0);

    // Update status icon color
    const statusIcon = document.getElementById("status-icon");
    if (data.indexer_status === "running") {
      statusIcon.className = "bg-green-100 p-3 rounded-full";
      statusIcon.innerHTML =
        '<svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6 text-green-600" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" /></svg>';
    } else {
      statusIcon.className = "bg-red-100 p-3 rounded-full";
      statusIcon.innerHTML =
        '<svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6 text-red-600" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>';
    }
  } catch (error) {
    console.error("Error loading stats:", error);
    throw error; // Re-throw to be caught by refreshAllData
  }
}

// Update stat with animation if value changed
function updateStatWithAnimation(elementId, newValue) {
  const element = document.getElementById(elementId);
  if (!element) {
    console.warn(`Element with id '${elementId}' not found`);
    return;
  }

  const oldValue = element.textContent.trim();

  // Only animate if value actually changed and it's not the initial load
  if (oldValue !== newValue && oldValue !== "..." && oldValue !== "") {
    element.classList.add("updating");
    setTimeout(() => {
      element.classList.remove("updating");
    }, 300);
  }

  element.textContent = newValue;
}

// Load recent blocks
async function loadRecentBlocks() {
  try {
    console.log('Loading recent blocks...');
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 10000); // 10 second timeout

    const response = await fetch(`${API_BASE}/blocks?per_page=5`, { signal: controller.signal });
    clearTimeout(timeoutId);

    console.log('Blocks response:', response.status);

    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }

    const data = await response.json();
    console.log('Blocks data:', data);

    if (data.blocks && data.blocks.length > 0) {
      const blocksList = document.getElementById("recent-blocks");

      // Build new content in a document fragment to avoid reflows
      const fragment = document.createDocumentFragment();

      for (const block of data.blocks) {
        const row = document.createElement("tr");
        row.className = "hover:bg-gray-50 fade-in";
        row.innerHTML = `
                      <td class="px-3 py-4 whitespace-nowrap text-left">
                          <a href="/blocks.html?number=${block.number
          }" class="text-blue-600 hover:text-blue-900">${block.number
          }</a>
                      </td>
                      <td class="px-3 py-4 whitespace-nowrap text-left">${block.transaction_count
          }</td>
                      <td class="px-3 py-4 whitespace-nowrap text-left">${formatNumber(
            block.gas_used || 0
          )}</td>
                      <td class="px-3 py-4 whitespace-nowrap text-right">${formatTimestamp(
            block.timestamp
          )}</td>
                  `;
        fragment.appendChild(row);
      }

      // Check if content has actually changed
      const newContent = fragment.cloneNode(true);
      const newHTML = Array.from(newContent.children).map(row => row.innerHTML).join('');
      const currentHTML = Array.from(blocksList.children).map(row => row.innerHTML).join('');

      // Only update and animate if content has changed
      if (newHTML !== currentHTML) {
        // Add updating class for smooth transition
        blocksList.classList.add("table-updating");

        // Replace content in one operation to minimize reflow
        blocksList.innerHTML = "";
        blocksList.appendChild(fragment);

        // Remove updating class
        setTimeout(() => {
          blocksList.classList.remove("table-updating");
        }, 100);
      }

      // Use block data for charts
      console.log('Creating charts with', data.blocks.length, 'blocks');
      createGasChart(data.blocks.slice().reverse());
      createTxsChart(data.blocks.slice().reverse());
    } else {
      console.log('No blocks found, showing empty state');
      showEmptyBlocksTable();
    }
  } catch (error) {
    console.error("Error loading blocks:", error);
    showEmptyBlocksTable();
    throw error; // Re-throw to be caught by refreshAllData
  }
}

// Load recent transactions
async function loadRecentTransactions() {
  try {
    const response = await fetch(`${API_BASE}/transactions?per_page=5`);

    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }

    const data = await response.json();

    const txsList = document.getElementById("recent-txs");

    if (data.transactions && data.transactions.length > 0) {
      // Build new content in a document fragment to avoid reflows
      const fragment = document.createDocumentFragment();

      for (const tx of data.transactions) {
        const row = document.createElement("tr");
        row.className = "hover:bg-gray-50 fade-in";
        row.innerHTML = `
                      <td class="px-3 py-4 whitespace-nowrap text-sm font-mono truncate">
                          <a href="/transactions.html?hash=${tx.hash
          }" class="hash-link">${truncateHash(
            tx.hash
          )}</a>
                      </td>
                      <td class="px-3 py-4 whitespace-nowrap text-sm font-mono truncate">
                          <a href="/accounts.html?address=${tx.from_address
          }" class="hash-link">${truncateHash(
            tx.from_address
          )}</a>
                      </td>
                      <td class="px-3 py-4 whitespace-nowrap text-sm font-mono truncate">
                          ${tx.to_address
            ? `<a href="/accounts.html?address=${tx.to_address
            }" class="hash-link">${truncateHash(
              tx.to_address
            )}</a>`
            : '<span class="text-gray-500 text-sm">Contract Creation</span>'
          }
                      </td>
                      <td class="px-3 py-4 whitespace-nowrap text-sm font-mono text-right">${formatEth(
            tx.value
          )}</td>
                  `;
        fragment.appendChild(row);
      }

      // Check if content has actually changed
      const newContent = fragment.cloneNode(true);
      const newHTML = Array.from(newContent.children).map(row => row.innerHTML).join('');
      const currentHTML = Array.from(txsList.children).map(row => row.innerHTML).join('');

      // Only update and animate if content has changed
      if (newHTML !== currentHTML) {
        // Add updating class for smooth transition
        txsList.classList.add("table-updating");

        // Replace content in one operation to minimize reflow
        txsList.innerHTML = "";
        txsList.appendChild(fragment);

        // Remove updating class
        setTimeout(() => {
          txsList.classList.remove("table-updating");
        }, 100);
      }
    } else {
      showEmptyTransactionsTable();
    }
  } catch (error) {
    console.error("Error loading transactions:", error);
    showEmptyTransactionsTable();
    throw error; // Re-throw to be caught by refreshAllData
  }
}

// Load live transactions (optimized for frequent updates)
async function loadLiveTransactions() {
  try {
    const response = await fetch(`${API_BASE}/transactions/live`);

    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }

    const data = await response.json();

    const txsList = document.getElementById("recent-txs");

    if (data.transactions && data.transactions.length > 0) {
      // Build new content in a document fragment to avoid reflows
      const fragment = document.createDocumentFragment();

      for (const tx of data.transactions) {
        const row = document.createElement("tr");
        row.className = "hover:bg-gray-50 fade-in";
        row.innerHTML = `
                      <td class="px-3 py-4 whitespace-nowrap text-sm font-mono truncate">
                          <a href="/transactions.html?hash=${tx.hash
          }" class="hash-link">${truncateHash(
            tx.hash
          )}</a>
                      </td>
                      <td class="px-3 py-4 whitespace-nowrap text-sm font-mono truncate">
                          <a href="/accounts.html?address=${tx.from_address
          }" class="hash-link">${truncateHash(
            tx.from_address
          )}</a>
                      </td>
                      <td class="px-3 py-4 whitespace-nowrap text-sm font-mono truncate">
                          ${tx.to_address
            ? `<a href="/accounts.html?address=${tx.to_address
            }" class="hash-link">${truncateHash(
              tx.to_address
            )}</a>`
            : '<span class="text-gray-500 text-sm">Contract Creation</span>'
          }
                      </td>
                      <td class="px-3 py-4 whitespace-nowrap text-sm font-mono text-right">${formatEth(
            tx.value
          )}</td>
                  `;
        fragment.appendChild(row);
      }

      // Check if content has actually changed
      const newContent = fragment.cloneNode(true);
      const newHTML = Array.from(newContent.children).map(row => row.innerHTML).join('');
      const currentHTML = Array.from(txsList.children).map(row => row.innerHTML).join('');

      // Only update and animate if content has changed
      if (newHTML !== currentHTML) {
        // Add updating class for smooth transition
        txsList.classList.add("table-updating");

        // Replace content in one operation to minimize reflow
        txsList.innerHTML = "";
        txsList.appendChild(fragment);

        // Remove updating class
        setTimeout(() => {
          txsList.classList.remove("table-updating");
        }, 100);
      }
    } else {
      showEmptyTransactionsTable();
    }
  } catch (error) {
    console.error("Error loading live transactions:", error);
    showEmptyTransactionsTable();
    throw error;
  }
}

// Load only transaction and account statistics (frequent updates)
async function loadTransactionStats() {
  try {
    const response = await fetch(`${API_BASE}/stats`);

    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }

    const data = await response.json();

    // Update transaction-related stats with animation for changes
    updateStatWithAnimation('total-indexed-txs', formatNumber(data.real_transactions_indexed));
    updateStatWithAnimation('total-blockchain-txs', formatNumber(data.total_blockchain_transactions));
    updateStatWithAnimation('total-indexed-accounts', formatNumber(data.total_accounts));
    updateStatWithAnimation('tx-indexed', formatNumber(data.real_transactions_indexed));
    updateStatWithAnimation('tx-declared', formatNumber(data.total_transactions_declared));

  } catch (error) {
    console.error("Error loading transaction stats:", error);
    throw error;
  }
}

// Load block-related statistics and other data (less frequent updates)
async function loadBlockStats() {
  try {
    const response = await fetch(`${API_BASE}/stats`);

    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }

    const data = await response.json();

    // Update only block-related and other stats
    const elements = {
      'total-blocks': data.total_blocks,
      'latest-block': data.latest_block_number,
      'gas-price': data.avg_gas_price ? `${data.avg_gas_price} gwei` : 'N/A',
      'block-time': data.avg_block_time ? `${data.avg_block_time}s` : 'N/A'
    };

    for (const [id, value] of Object.entries(elements)) {
      const element = document.getElementById(id);
      if (element && value !== undefined) {
        const newValue = typeof value === 'number' ? formatNumber(value) : value;
        if (element.textContent !== newValue) {
          element.textContent = newValue;
        }
      }
    }

    // Update network sync progress
    if (data.network_latest_block && data.latest_block_number) {
      updateNetworkProgress(data.latest_block_number, data.network_latest_block);
    }
  } catch (error) {
    console.error("Error loading block stats:", error);
    throw error;
  }
}

// Update network sync progress
function updateNetworkProgress(latestBlock, networkBlock) {
  const progressBar = document.getElementById('network-progress-bar');
  const progressText = document.getElementById('network-progress-text');

  if (progressBar && progressText) {
    const percentage = Math.min((latestBlock / networkBlock) * 100, 100);
    progressBar.style.width = `${percentage}%`;
    progressText.textContent = `${percentage.toFixed(2)}% (${latestBlock.toLocaleString()} / ${networkBlock.toLocaleString()})`;
  }
}

// Global variables for delta updates
let lastBlockNumber = 0;
let lastTransactionHash = '';

// Load initial data
document.addEventListener("DOMContentLoaded", () => {
  // Initialize data on first load
  initializeData();

  // After initialization, refresh all data every 1 second using delta updates
  const refreshInterval = 1000; // 1 second
  setTimeout(() => {
    setInterval(refreshAllData, refreshInterval);
  }, 2000); // Wait 2 seconds before starting delta updates
});

// Show/hide frequent data updating indicator
function setFrequentDataUpdating(isUpdating) {
  // Update stats indicators for frequently updated items
  const statsElements = ['latest-network-block', 'latest-indexed-block', 'total-network-txs', 'total-indexed-txs', 'total-blockchain-txs', 'total-network-accounts', 'total-indexed-accounts', 'tx-indexed', 'tx-declared'];
  statsElements.forEach(id => {
    const element = document.getElementById(id);
    if (element) {
      // Find the parent container with text content
      const parentContainer = element.closest('.stat-card');
      if (parentContainer) {
        const textContainer = parentContainer.querySelector('p.text-sm.font-medium');
        if (textContainer && isUpdating) {
          if (!textContainer.querySelector('.updating-indicator')) {
            const indicator = document.createElement('span');
            indicator.className = 'updating-indicator inline-block w-1 h-1 bg-green-500 rounded-full ml-1 animate-pulse';
            textContainer.appendChild(indicator);
          }
        } else if (textContainer) {
          const indicator = textContainer.querySelector('.updating-indicator');
          if (indicator) {
            indicator.remove();
          }
        }
      } else {
        // For elements not in stat-cards (like tx-indexed, tx-declared)
        const parentElement = element.parentElement;
        if (parentElement && isUpdating) {
          if (!parentElement.querySelector('.updating-indicator')) {
            const indicator = document.createElement('span');
            indicator.className = 'updating-indicator inline-block w-1 h-1 bg-green-500 rounded-full ml-1 animate-pulse';
            parentElement.appendChild(indicator);
          }
        } else if (parentElement) {
          const indicator = parentElement.querySelector('.updating-indicator');
          if (indicator) {
            indicator.remove();
          }
        }
      }
    }
  });
}

// Show empty state for blocks table when API is unavailable
function showEmptyBlocksTable() {
  const blocksList = document.getElementById("recent-blocks");
  if (blocksList) {
    blocksList.innerHTML = `
      <tr>
        <td colspan="4" class="px-6 py-8 text-center text-gray-500">
          <div class="flex flex-col items-center">
            <svg class="w-8 h-8 mb-2 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"></path>
            </svg>
            <p>No blocks data available</p>
          </div>
        </td>
      </tr>
    `;
  }
}

// Show empty state for transactions table when API is unavailable
function showEmptyTransactionsTable() {
  const txsList = document.getElementById("recent-txs");
  if (txsList) {
    txsList.innerHTML = `
      <tr>
        <td colspan="4" class="px-6 py-8 text-center text-gray-500">
          <div class="flex flex-col items-center">
            <svg class="w-8 h-8 mb-2 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"></path>
            </svg>
            <p>No transactions data available</p>
          </div>
        </td>
      </tr>
    `;
  }
}

// Show default empty charts when no data is available
function showEmptyCharts() {
  // Initialize empty charts without creating them with fake data
  // This prevents the issue of showing fixed gas_limit values
  const gasCtx = document.getElementById("gas-chart");
  const txsCtx = document.getElementById("txs-chart");

  if (gasCtx && !gasChart) {
    // Create empty gas chart without data
    gasChart = new Chart(gasCtx.getContext("2d"), {
      type: "line",
      data: {
        labels: [],
        datasets: [
          {
            label: "Gas Used",
            data: [],
            borderColor: "rgba(59, 130, 246, 1)",
            backgroundColor: "rgba(59, 130, 246, 0.1)",
            fill: true,
            tension: 0.4,
          },
          {
            label: "Gas Limit",
            data: [],
            borderColor: "rgba(156, 163, 175, 1)",
            backgroundColor: "rgba(156, 163, 175, 0.1)",
            fill: false,
            tension: 0.4,
            borderDash: [5, 5],
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: true,
        animation: { duration: 0 },
        scales: {
          y: {
            beginAtZero: true,
            title: { display: true, text: 'Gas Units' }
          },
          x: {
            title: { display: true, text: 'Recent Blocks' }
          }
        },
        plugins: {
          legend: { display: true },
          tooltip: {
            callbacks: {
              title: function (context) {
                const blockIndex = context[0].dataIndex;
                const blocks = this.chart.blockData || [];
                return blocks[blockIndex] ? `Block #${blocks[blockIndex].number}` : '';
              },
              label: function (context) {
                const blockIndex = context.dataIndex;
                const blocks = this.chart.blockData || [];
                const block = blocks[blockIndex];
                if (!block) return `${context.dataset.label}: ${context.parsed.y.toLocaleString()}`;

                const efficiency = block.gas_limit > 0 ? ((block.gas_used / block.gas_limit) * 100).toFixed(2) : 0;
                return [
                  `${context.dataset.label}: ${context.parsed.y.toLocaleString()}`,
                  `Efficiency: ${efficiency}%`,
                  `Transactions: ${block.transaction_count}`
                ];
              }
            }
          }
        }
      },
    });
    gasChart.blockData = [];
  }

  if (txsCtx && !txsChart) {
    // Create empty transactions chart without data
    txsChart = new Chart(txsCtx.getContext("2d"), {
      type: "bar",
      data: {
        labels: [],
        datasets: [{
          label: "Transaction Count",
          data: [],
          backgroundColor: "rgba(16, 185, 129, 0.6)",
          borderColor: "rgba(16, 185, 129, 1)",
          borderWidth: 1,
        }],
      },
      options: {
        responsive: true,
        maintainAspectRatio: true,
        animation: { duration: 0 },
        scales: {
          y: {
            beginAtZero: true,
            title: { display: true, text: 'Transaction Count' }
          },
          x: {
            title: { display: true, text: 'Recent Blocks' }
          }
        },
        plugins: {
          legend: { display: true },
          tooltip: {
            callbacks: {
              title: function (context) {
                const blockIndex = context[0].dataIndex;
                const blocks = this.chart.blockData || [];
                return blocks[blockIndex] ? `Block #${blocks[blockIndex].number}` : '';
              },
              label: function (context) {
                const blockIndex = context.dataIndex;
                const blocks = this.chart.blockData || [];
                const block = blocks[blockIndex];
                const hasTransactions = this.chart.hasTransactions;

                if (!block) return `${context.dataset.label}: ${context.parsed.y.toLocaleString()}`;

                if (hasTransactions) {
                  return `Transactions: ${block.transaction_count}`;
                } else {
                  const gasPercentage = block.gas_limit > 0 ? ((block.gas_used / block.gas_limit) * 100).toFixed(2) : 0;
                  return [
                    `Gas Usage: ${gasPercentage}%`,
                    `Gas Used: ${block.gas_used}`,
                    `Gas Limit: ${block.gas_limit}`
                  ];
                }
              }
            }
          }
        }
      },
    });
    txsChart.blockData = [];
    txsChart.hasTransactions = true;
  }
}

// Create gas usage chart
function createGasChart(blocks) {
  if (!blocks || blocks.length === 0) return;

  const labels = blocks.map((block) => `#${block.number}`);
  const gasUsedData = blocks.map((block) => block.gas_used);
  const gasLimitData = blocks.map((block) => block.gas_limit);

  // If chart exists, update data instead of recreating
  if (gasChart) {
    gasChart.data.labels = labels;
    gasChart.data.datasets[0].data = gasUsedData;
    gasChart.data.datasets[1].data = gasLimitData;
    // Store block data for tooltips
    gasChart.blockData = blocks;
    gasChart.update('none'); // Update without animation
    return;
  }

  // Create new chart only if it doesn't exist
  const gasCtx = document.getElementById("gas-chart").getContext("2d");
  gasChart = new Chart(gasCtx, {
    type: "line",
    data: {
      labels: labels,
      datasets: [
        {
          label: "Gas Used",
          data: gasUsedData,
          borderColor: "rgba(59, 130, 246, 1)",
          backgroundColor: "rgba(59, 130, 246, 0.1)",
          fill: true,
          tension: 0.4,
        },
        {
          label: "Gas Limit",
          data: gasLimitData,
          borderColor: "rgba(156, 163, 175, 1)",
          backgroundColor: "rgba(156, 163, 175, 0.1)",
          fill: false,
          tension: 0.4,
          borderDash: [5, 5],
        },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: true,
      animation: {
        duration: 0 // Disable animations for smoother updates
      },
      scales: {
        y: {
          beginAtZero: true,
          title: {
            display: true,
            text: 'Gas Units'
          }
        },
        x: {
          title: {
            display: true,
            text: 'Recent Blocks'
          }
        }
      },
      plugins: {
        legend: {
          display: true,
        },
        tooltip: {
          callbacks: {
            title: function (context) {
              const blockIndex = context[0].dataIndex;
              const blocks = this.chart.blockData || [];
              return blocks[blockIndex] ? `Block #${blocks[blockIndex].number}` : '';
            },
            label: function (context) {
              const blockIndex = context.dataIndex;
              const blocks = this.chart.blockData || [];
              const block = blocks[blockIndex];
              if (!block) return `${context.dataset.label}: ${context.parsed.y.toLocaleString()}`;

              const efficiency = block.gas_limit > 0 ? ((block.gas_used / block.gas_limit) * 100).toFixed(2) : 0;
              return [
                `${context.dataset.label}: ${context.parsed.y.toLocaleString()}`,
                `Efficiency: ${efficiency}%`,
                `Transactions: ${block.transaction_count}`
              ];
            }
          }
        }
      }
    },
  });

  // Store block data for tooltips
  gasChart.blockData = blocks;
}

// Update progress bar with nested indicators
function updateProgressBar(currentBlock, networkBlock, startBlock = 0) {
  const progressBar = document.getElementById('progress-bar');
  const currentBlockProgress = document.getElementById('current-block-progress');
  const startBlockProgress = document.getElementById('start-block-progress');
  const blockProgressIndicator = document.getElementById('block-progress-indicator');

  if (networkBlock > 0 && progressBar && currentBlockProgress) {
    // Calculate progress from start block to network block
    const totalBlocks = networkBlock - startBlock;
    const processedBlocks = Math.max(0, currentBlock - startBlock);
    const percentage = totalBlocks > 0 ? Math.min((processedBlocks / totalBlocks) * 100, 100) : 0;

    progressBar.style.width = `${percentage}%`;

    // Update block progress indicator
    if (blockProgressIndicator) {
      blockProgressIndicator.textContent = `${percentage.toFixed(1)}% (${processedBlocks.toLocaleString()} / ${totalBlocks.toLocaleString()})`;
    }

    currentBlockProgress.textContent = `Block ${currentBlock.toLocaleString()}`;

    // Update start block display
    if (startBlockProgress) {
      startBlockProgress.textContent = `Block ${startBlock.toLocaleString()}`;
    }
  }
}

// Create transactions chart
function createTxsChart(blocks) {
  if (!blocks || blocks.length === 0) return;

  const labels = blocks.map((block) => `#${block.number}`);
  const txCtx = document.getElementById("txs-chart").getContext("2d");
  const txData = blocks.map((block) => block.transaction_count);

  // If no transactions, show gas usage as percentage
  const hasTransactions = txData.some(value => value > 0);
  let chartData, chartLabel, chartColor;

  if (hasTransactions) {
    chartData = txData;
    chartLabel = "Transaction Count";
    chartColor = "rgba(16, 185, 129, 0.6)";
  } else {
    // Show gas usage percentage when no transactions
    chartData = blocks.map((block) => block.gas_limit > 0 ? (block.gas_used / block.gas_limit) * 100 : 0);
    chartLabel = "Gas Usage %";
    chartColor = "rgba(156, 163, 175, 0.6)";
  }

  // If chart exists, update data instead of recreating
  if (txsChart) {
    txsChart.data.labels = labels;
    txsChart.data.datasets[0].data = chartData;
    txsChart.data.datasets[0].label = chartLabel;
    txsChart.data.datasets[0].backgroundColor = chartColor;
    txsChart.data.datasets[0].borderColor = chartColor.replace('0.6', '1');
    txsChart.options.scales.y.title.text = hasTransactions ? 'Transaction Count' : 'Gas Usage (%)';
    txsChart.options.scales.y.max = hasTransactions ? undefined : 100;
    // Store block data for tooltips
    txsChart.blockData = blocks;
    txsChart.hasTransactions = hasTransactions;
    txsChart.update('none'); // Update without animation
    return;
  }

  // Create new chart only if it doesn't exist
  txsChart = new Chart(txCtx, {
    type: "bar",
    data: {
      labels: labels,
      datasets: [
        {
          label: chartLabel,
          data: chartData,
          backgroundColor: chartColor,
          borderColor: chartColor.replace('0.6', '1'),
          borderWidth: 1,
        },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: true,
      animation: {
        duration: 0 // Disable animations for smoother updates
      },
      scales: {
        y: {
          beginAtZero: true,
          max: hasTransactions ? undefined : 100,
          title: {
            display: true,
            text: hasTransactions ? 'Transaction Count' : 'Gas Usage (%)'
          }
        },
        x: {
          title: {
            display: true,
            text: 'Recent Blocks'
          }
        }
      },
      plugins: {
        legend: {
          display: true,
        },
        tooltip: {
          callbacks: {
            title: function (context) {
              const blockIndex = context[0].dataIndex;
              const blocks = this.chart.blockData || [];
              return blocks[blockIndex] ? `Block #${blocks[blockIndex].number}` : '';
            },
            label: function (context) {
              const blockIndex = context.dataIndex;
              const blocks = this.chart.blockData || [];
              const block = blocks[blockIndex];
              const hasTransactions = this.chart.hasTransactions;

              if (!block) return `${context.dataset.label}: ${context.parsed.y.toLocaleString()}`;

              if (hasTransactions) {
                return `Transactions: ${block.transaction_count}`;
              } else {
                const gasPercentage = block.gas_limit > 0 ? ((block.gas_used / block.gas_limit) * 100).toFixed(2) : 0;
                return [
                  `Gas Usage: ${gasPercentage}%`,
                  `Gas Used: ${block.gas_used}`,
                  `Gas Limit: ${block.gas_limit}`
                ];
              }
            }
          }
        }
      }
    },
  });

  // Store block data for tooltips
  txsChart.blockData = blocks;
  txsChart.hasTransactions = hasTransactions;
}

// Handle search
function handleSearchKeyPress(e) {
  if (e.key === "Enter") {
    const query = document.getElementById("search-input").value.trim();
    if (query) {
      window.location.href = `/search.html?q=${encodeURIComponent(
        query
      )}`;
    }
  }
}

// Load recent blocks using delta updates (optimized)
async function loadRecentBlocksDelta() {
  try {
    const response = await fetch(`${API_BASE}/blocks/since?since=${lastBlockNumber}`);

    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }

    const data = await response.json();

    if (data.blocks && data.blocks.length > 0) {
      const blocksList = document.getElementById("recent-blocks");

      // Update lastBlockNumber to the highest block number received
      if (data.blocks.length > 0) {
        lastBlockNumber = Math.max(...data.blocks.map(b => b.number));
      }

      // Create new rows for the new blocks
      const newRows = [];
      for (const block of data.blocks) {
        const row = document.createElement("tr");
        row.className = "hover:bg-gray-50";
        row.innerHTML = `
          <td class="px-3 py-4 whitespace-nowrap text-left">
            <a href="/blocks.html?number=${block.number}" class="text-blue-600 hover:text-blue-900">${block.number}</a>
          </td>
          <td class="px-3 py-4 whitespace-nowrap text-left">${block.transaction_count}</td>
          <td class="px-3 py-4 whitespace-nowrap text-left">${formatNumber(block.gas_used || 0)}</td>
          <td class="px-3 py-4 whitespace-nowrap text-right">${formatTimestamp(block.timestamp)}</td>
        `;
        newRows.push(row);
      }

      // Insert new rows at the beginning and remove excess rows from the end
      const maxRows = 5;

      // Insert new rows at the beginning
      for (let i = newRows.length - 1; i >= 0; i--) {
        blocksList.insertBefore(newRows[i], blocksList.firstChild);
      }

      // Remove excess rows from the end to maintain max 5 rows
      while (blocksList.children.length > maxRows) {
        blocksList.removeChild(blocksList.lastChild);
      }

      // Update charts with new blocks if charts exist and we have new data
      if (data.blocks.length > 0 && gasChart && txsChart) {
        // Get current chart data (in chronological order - oldest first)
        const currentBlockData = gasChart.blockData || [];

        // data.blocks comes in descending order (newest first) from API
        // We need to reverse it first to get chronological order
        const newBlocksChronological = [...data.blocks].reverse();

        // Combine new blocks (chronological) with existing blocks (chronological)
        const updatedBlockData = [...currentBlockData, ...newBlocksChronological];

        // Keep only the 5 most recent blocks (take from the end since they're in chronological order)
        const chartBlockData = updatedBlockData.slice(-5);

        // Charts need blocks in chronological order (oldest first) - which we already have
        createGasChart(chartBlockData);
        createTxsChart(chartBlockData);
      }
    } else if (lastBlockNumber === 0) {
      // First load - get initial data
      await loadRecentBlocks();
    } else {
      // No new blocks but we need to ensure we have 5 blocks displayed
      // Check if we have less than 5 blocks in the table
      const blocksList = document.getElementById("recent-blocks");
      if (blocksList && blocksList.children.length < 5) {
        // Force a full reload to ensure we have 5 blocks
        await loadRecentBlocks();
      }
    }
  } catch (error) {
    console.error("Error loading recent blocks delta:", error);
    // On error, fall back to full load if this is first attempt
    if (lastBlockNumber === 0) {
      try {
        await loadRecentBlocks();
      } catch (fallbackError) {
        console.error("Fallback load also failed:", fallbackError);
        showEmptyBlocksTable();
      }
    } else {
      showEmptyBlocksTable();
    }
    throw error;
  }
}

// Load recent transactions using delta updates (optimized)
async function loadRecentTransactionsDelta() {
  try {
    console.log('Fetching transactions since:', lastTransactionHash);
    const response = await fetch(`${API_BASE}/transactions/since?since=${encodeURIComponent(lastTransactionHash)}`);

    if (!response.ok) {
      throw new Error(`API returned ${response.status}`);
    }

    const data = await response.json();
    console.log('Transactions delta response:', data.transactions?.length || 0, 'transactions');

    if (data.transactions && data.transactions.length > 0) {
      const txsList = document.getElementById("recent-txs");

      // Update lastTransactionHash to the most recent transaction
      if (data.transactions.length > 0) {
        const oldHash = lastTransactionHash;
        lastTransactionHash = data.transactions[0].hash;
        console.log('Updated lastTransactionHash from', oldHash.substring(0, 10) + '...', 'to', lastTransactionHash.substring(0, 10) + '...');
      }

      // Create new rows for the new transactions
      const newRows = [];
      for (const tx of data.transactions) {
        const row = document.createElement("tr");
        row.className = "hover:bg-gray-50";
        row.innerHTML = `
          <td class="px-3 py-4 whitespace-nowrap text-sm font-mono truncate">
            <a href="/transactions.html?hash=${tx.hash}" class="hash-link">${truncateHash(tx.hash)}</a>
          </td>
          <td class="px-3 py-4 whitespace-nowrap text-sm font-mono truncate">
            <a href="/accounts.html?address=${tx.from_address}" class="hash-link">${truncateHash(tx.from_address)}</a>
          </td>
          <td class="px-3 py-4 whitespace-nowrap text-sm font-mono truncate">
            ${tx.to_address
            ? `<a href="/accounts.html?address=${tx.to_address}" class="hash-link">${truncateHash(tx.to_address)}</a>`
            : '<span class="text-gray-500 text-sm">Contract Creation</span>'
          }
          </td>
          <td class="px-3 py-4 whitespace-nowrap text-sm font-mono text-right">${formatEth(tx.value)}</td>
        `;
        newRows.push(row);
      }

      // Insert new rows at the beginning and remove excess rows from the end
      const maxRows = 5;

      // Insert new rows at the beginning
      for (let i = newRows.length - 1; i >= 0; i--) {
        txsList.insertBefore(newRows[i], txsList.firstChild);
      }

      // Remove excess rows from the end to maintain max 5 rows
      while (txsList.children.length > maxRows) {
        txsList.removeChild(txsList.lastChild);
      }
    } else if (lastTransactionHash === '') {
      // First load - get initial data
      await loadRecentTransactions();
    }
  } catch (error) {
    console.error("Error loading recent transactions delta:", error);
    // On error, fall back to full load if this is first attempt
    if (lastTransactionHash === '') {
      try {
        await loadRecentTransactions();
      } catch (fallbackError) {
        console.error("Fallback load also failed:", fallbackError);
        showEmptyTransactionsTable();
      }
    } else {
      showEmptyTransactionsTable();
    }
    throw error;
  }
}
