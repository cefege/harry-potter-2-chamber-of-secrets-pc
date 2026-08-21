//===============================================================================
//  #22 Circe
//===============================================================================

class  WCCirce extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardCirceTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Circesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardCirceBigTexture FILE=TEXTURES\menu\Folio\Cards\Circebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Circe";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=22
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE110"
    skin(0)=Texture'HProps.skins.WizardCardCirceTex0'
	textureBig=Texture'WizCardCirceBigTexture'
	strDescriptionId="WizCard_0018"
}
