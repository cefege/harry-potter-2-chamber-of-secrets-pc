//===============================================================================
// #55	Honoria Nutcombe
//===============================================================================

class  WCNutcombe extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardNutcombeTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Nutcombesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardNutcombeBigTexture FILE=TEXTURES\menu\Folio\Cards\Nutcombebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Honoria Nutcombe";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=55
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE090"
    skin(0)=Texture'HProps.skins.WizardCardNutcombeTex0'
	textureBig=Texture'WizCardNutcombeBigTexture'
	strDescriptionId="WizCard_0091"
}
