<?php

/**
 * Example: Get Profile Information
 * 
 * This script demonstrates how to retrieve the account profile information.
 */

use Vipayment\Vipayment;

/** @var Vipayment $vipayment */
$vipayment = require __DIR__ . '/config.php';

echo "=== Get Profile ===\n";

$response = $vipayment->profile();

if ($response['status']) {
    echo "Success!\n";
    print_r($response['data']);
} else {
    echo "Failed: " . $response['message'] . "\n";
}
