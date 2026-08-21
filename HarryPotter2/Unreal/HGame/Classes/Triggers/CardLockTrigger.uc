//===============================================================================
//  CardLockTrigger
//
//  When triggered, CardLockTrigger will send out an event for each silver lock 
//  that the player has.  It will then remove those silver locks from the 
//  player's "inventory."  If the player has more than one lock, the events will 
//  be sent out sequentially with a pause of fLockWaitTimeX between each event.
//
//  The name of the events sent out and the time to wait between each event
//  can be customized from within UnrealEd if need be (under "CardLockTrigger"
//  properties).
//  
//===============================================================================



class CardLockTrigger extends trigger;

var StatusGroupLocks sgLocks;
var StatusItemLock1  siLock1;
var StatusItemLock2  siLock2;
var StatusItemLock3  siLock3;
var StatusItemLock4  siLock4;

// Possible Events sent out when triggered
var() name nameLock1Event;     // Sent if player currently has lock1
var() name nameLock2Event;     // Sent if player currently has lock2
var() name nameLock3Event;     // Sent if player currently has lock3
var() name nameLock4Event;     // Sent if player currently has lock4

// Wait time after corresponding event is sent out
var() float fLock1WaitTime;    // Pause time after Lock1 event sent out
var() float fLock2WaitTime;    // Pause time after Lock1 event sent out
var() float fLock3WaitTime;    // Pause time after Lock1 event sent out
var() float fLock4WaitTime;    // Pause time after Lock1 event sent out



state NormalTrigger
{
    event Activate(actor Other, pawn EventInstigator)
    {
        Super.Activate(Other, EventInstigator);

	    if (Other != Level.PlayerHarryActor )
            return;

        GoToState('SendLockMessages');
    }
}

state SendLockMessages
{

    function BeginState()
    {
        sgLocks = StatusGroupLocks(Harry(Level.PlayerHarryActor).managerStatus.GetStatusGroup(class'StatusGroupLocks'));
        siLock1 = StatusItemLock1(sgLocks.GetStatusItem(class'StatusItemLock1'));
        siLock2 = StatusItemLock2(sgLocks.GetStatusItem(class'StatusItemLock2'));
        siLock3 = StatusItemLock3(sgLocks.GetStatusItem(class'StatusItemLock3'));
        siLock4 = StatusItemLock4(sgLocks.GetStatusItem(class'StatusItemLock4'));
    }

begin:
    if (siLock1.nCount >= 1)
    {
        TriggerEvent(nameLock1Event, none, none );
        Sleep(fLock1WaitTime);
        siLock1.SetCount(0);       
    }

    if (siLock2.nCount >= 1)
    {        
        TriggerEvent(nameLock2Event, none, none );
        Sleep(fLock2WaitTime);
        siLock2.SetCount(0);   
    }

    if (siLock3.nCount >= 1)
    {        
        TriggerEvent(nameLock3Event, none, none );
        Sleep(fLock3WaitTime);
        siLock3.SetCount(0);   
    }

    if (siLock4.nCount >= 1)
    {        
        TriggerEvent(nameLock4Event, none, none );
        Sleep(fLock4WaitTime);
        siLock4.SetCount(0);
    }  
    
    GoToState('NormalTrigger');
}

//*****************************************************************************
defaultproperties
{
   // Possible Events sent out
    nameLock1Event=CardLock1
    nameLock2Event=CardLock2
    nameLock3Event=CardLock3
    nameLock4Event=CardLock4

    // Wait time after corresponding event is sent out
    fLock1WaitTime=1.0
    fLock2WaitTime=1.0
    fLock3WaitTime=1.0
    fLock4WaitTime=1.0
}