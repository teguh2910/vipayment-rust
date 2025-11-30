<?php

use PHPUnit\Framework\TestCase;
use Vipayment\Vipayment;

class VipaymentTest extends TestCase
{
    public function testInstantiation()
    {
        $apiId = 'test_id';
        $apiKey = 'test_key';
        $vipayment = new Vipayment($apiId, $apiKey);

        $this->assertInstanceOf(Vipayment::class, $vipayment);
    }

    public function testProfileStructure()
    {
        // We cannot easily test the actual API call without mocking, 
        // but we can test if the method exists and returns expected structure if we could mock the private connect method.
        // For now, we just check if the method is callable.
        $apiId = 'test_id';
        $apiKey = 'test_key';
        $vipayment = new Vipayment($apiId, $apiKey);
        
        $this->assertTrue(method_exists($vipayment, 'profile'));
    }
}
