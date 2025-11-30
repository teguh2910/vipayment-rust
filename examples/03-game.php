<?php

/**
 * Example: Game & Streaming Services
 * 
 * This script demonstrates:
 * 1. Checking available game services.
 * 2. Ordering a game top-up.
 * 3. Checking the status of a game order.
 */

use Vipayment\Vipayment;

/** @var Vipayment $vipayment */
$vipayment = require __DIR__ . '/config.php';

// -----------------------------------------------------------------------------
// 1. Check Available Game Services
// -----------------------------------------------------------------------------
echo "\n=== 1. Check Available Game Services ===\n";

// Filter by game name (e.g., 'Mobile Legends') and status ('available')
// Note: The first argument is 'filter_game' and second is 'filter_status'
$services = $vipayment->serviceGame('Mobile Legends', 'available');

if ($services['status']) {
    echo "Found " . count($services['data']) . " services.\n";
    
    // Display first 3 services as example
    $count = 0;
    foreach ($services['data'] as $service) {
        if ($count++ >= 3) break;
        echo "- [{$service['code']}] {$service['name']} (Price: {$service['price']})\n";
    }
} else {
    echo "Failed to get services: " . $services['message'] . "\n";
}

// -----------------------------------------------------------------------------
// 2. Order Game Top-up (Example)
// -----------------------------------------------------------------------------
echo "\n=== 2. Order Game Top-up ===\n";

// NOTE: Replace with valid data
$serviceCode = 'ML5'; 
$userId = '12345678';
$zoneId = '1234'; // Optional, depends on game

// Uncomment to run order
// $order = $vipayment->orderGame($serviceCode, $userId, $zoneId);
// if ($order['status']) {
//     echo "Order Success! TrxID: " . $order['data']['trxid'] . "\n";
// } else {
//     echo "Order Failed: " . $order['message'] . "\n";
// }

// -----------------------------------------------------------------------------
// 3. Check Order Status
// -----------------------------------------------------------------------------
echo "\n=== 3. Check Order Status ===\n";

// NOTE: Replace with valid Transaction ID
$trxId = 'TRX987654321';

// Uncomment to check status
// $status = $vipayment->statusOrderGame($trxId);
// if ($status['status']) {
//     echo "Status: " . $status['data'][0]['status'] . "\n";
// } else {
//     echo "Check Status Failed: " . $status['message'] . "\n";
// }
