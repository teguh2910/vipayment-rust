<?php

/**
 * Example: Prepaid Services (Pulsa, Data, PLN, etc.)
 * 
 * This script demonstrates:
 * 1. Checking available prepaid services.
 * 2. Ordering a prepaid service.
 * 3. Checking the status of an order.
 */

use Vipayment\Vipayment;

/** @var Vipayment $vipayment */
$vipayment = require __DIR__ . '/config.php';

// -----------------------------------------------------------------------------
// 1. Check Available Services
// -----------------------------------------------------------------------------
echo "\n=== 1. Check Available Services (Pulsa Reguler) ===\n";

// Filter by type: 'pulsa-reguler'
$services = $vipayment->servicePrepaid('type', 'pulsa-reguler');

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
// 2. Order Prepaid Service (Example)
// -----------------------------------------------------------------------------
echo "\n=== 2. Order Prepaid Service ===\n";

// NOTE: Replace with valid service code and destination number
$serviceCode = 'PLN5'; 
$destinationNumber = '1234567890';

// Uncomment to run order
// $order = $vipayment->orderPrepaid($serviceCode, $destinationNumber);
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
$trxId = 'TRX123456789';

// Uncomment to check status
// $status = $vipayment->statusOrderPrepaid($trxId);
// if ($status['status']) {
//     echo "Status: " . $status['data'][0]['status'] . "\n";
//     echo "SN: " . $status['data'][0]['sn'] . "\n";
// } else {
//     echo "Check Status Failed: " . $status['message'] . "\n";
// }
